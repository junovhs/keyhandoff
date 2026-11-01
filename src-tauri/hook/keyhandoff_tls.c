/*
 * KeyHandoff's TLS hook, preloaded by `keyhandoff run` into the programs it
 * starts (LD_PRELOAD). The program holds a placeholder that looks like the
 * key. When an outgoing TLS write contains one, this checks the connection's
 * certificate itself, against the system's trusted roots rather than the
 * program's (which code in the program could change), and only when the
 * certificate is valid for one of the key's hosts asks KeyHandoff for the
 * real key, swaps it into a copy of the bytes being encrypted, writes them
 * and wipes the copy. Anywhere else, the placeholder goes out as it is.
 *
 * It interposes OpenSSL's SSL_write and SSL_write_ex, resolving everything it
 * calls at run time, so it needs no OpenSSL headers and works with 1.1 and 3.
 * A program that links OpenSSL statically, or uses another TLS library, isn't
 * covered; its placeholder goes out unchanged and the server refuses it.
 *
 * KEYHANDOFF_HOOK ("<64 hex token>:<key socket>") is read once at load and
 * removed from the environment before the program's own code starts, then
 * put back for programs it starts (exec, posix_spawn, system, popen).
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <pthread.h>
#include <spawn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

typedef struct ssl_st SSL;
typedef struct x509_st X509;
typedef struct x509_store_st X509_STORE;
typedef struct x509_store_ctx_st X509_STORE_CTX;
typedef struct stack_st STACK;

#define ENV_NAME "KEYHANDOFF_HOOK"
#define TOKEN_HEX 64
#define MAX_SWAPS 64
#define MAX_HOSTS 512
#define SSL_CTRL_MODE 33
#define SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER 0x2L

extern char **environ;

static char hook_env[sizeof(ENV_NAME) + TOKEN_HEX + 1 + 108 + 1];
static char token[TOKEN_HEX + 1];
static char socket_path[108];
static int active;

struct swap {
    char *placeholder;
    size_t len;
    char hosts[MAX_HOSTS];
    char *ca;
};
static struct swap swaps[MAX_SWAPS];
static int swap_count = -1; /* -1: not asked yet */
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;

__attribute__((constructor)) static void hook_init(void) {
    const char *value = getenv(ENV_NAME);
    if (!value || strlen(value) < TOKEN_HEX + 2 || value[TOKEN_HEX] != ':' || strlen(value + TOKEN_HEX + 1) >= sizeof socket_path)
        return;
    memcpy(token, value, TOKEN_HEX);
    strcpy(socket_path, value + TOKEN_HEX + 1);
    snprintf(hook_env, sizeof hook_env, "%s=%s", ENV_NAME, value);
    unsetenv(ENV_NAME);
    active = 1;
}

/* ---- Finding OpenSSL ---- */

static void *find(const char *name) {
    void *p = dlsym(RTLD_NEXT, name);
    if (p) return p;
    /* Python and others load libssl privately (RTLD_LOCAL), out of RTLD_NEXT's reach. */
    static const char *libs[] = {"libssl.so.3", "libssl.so.1.1", "libssl.so", "libcrypto.so.3", "libcrypto.so.1.1", 0};
    for (int i = 0; libs[i]; i++) {
        void *h = dlopen(libs[i], RTLD_NOW | RTLD_NOLOAD);
        if (h && (p = dlsym(h, name))) return p;
    }
    return 0;
}

#define REAL(ret, name, args) \
    static ret(*real_##name) args; \
    if (!real_##name) real_##name = (ret(*) args)find(#name);

/* ---- Talking to KeyHandoff ---- */

/* Connect to KeyHandoff and send one line; returns the socket, or -1. */
static int send_line(const char *line) {
    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (fd < 0) return -1;
    struct sockaddr_un addr = {.sun_family = AF_UNIX};
    memcpy(addr.sun_path, socket_path, strlen(socket_path)); /* shorter than sun_path, checked at load */
    if (connect(fd, (struct sockaddr *)&addr, sizeof addr) < 0) {
        close(fd);
        return -1;
    }
    size_t sent = 0, n = strlen(line);
    while (sent < n) {
        ssize_t w = send(fd, line + sent, n - sent, MSG_NOSIGNAL);
        if (w <= 0) {
            close(fd);
            return -1;
        }
        sent += (size_t)w;
    }
    return fd;
}

/* Send one line and read one reply line into `out`. Returns its length, or -1. */
static int ask(const char *line, char *out, size_t cap) {
    int fd = send_line(line);
    if (fd < 0) return -1;
    size_t got = 0;
    while (got + 1 < cap) {
        ssize_t r = recv(fd, out + got, cap - 1 - got, 0);
        if (r <= 0) break;
        got += (size_t)r;
        if (memchr(out + got - r, '\n', (size_t)r)) break;
    }
    close(fd);
    out[got] = 0;
    char *end = memchr(out, '\n', got);
    if (!end) return -1;
    *end = 0;
    return (int)(end - out);
}

/* The run's placeholders, asked for once: "placeholder\thost,host\tca" per line. */
static void load_swaps(void) {
    if (swap_count >= 0) return;
    swap_count = 0;
    char request[160], *reply = malloc(64 * 1024);
    if (!reply) return;
    snprintf(request, sizeof request, "{\"hook\":\"%s\"}\n", token);
    int fd = send_line(request);
    if (fd < 0) {
        free(reply);
        return;
    }
    size_t got = 0;
    for (ssize_t r; got + 1 < 64 * 1024 && (r = recv(fd, reply + got, 64 * 1024 - 1 - got, 0)) > 0;) got += (size_t)r;
    close(fd);
    reply[got] = 0;
    for (char *line = strtok(reply, "\n"); line && swap_count < MAX_SWAPS; line = strtok(0, "\n")) {
        char *hosts = strchr(line, '\t');
        if (!hosts) continue;
        *hosts++ = 0;
        char *ca = strchr(hosts, '\t');
        if (ca) *ca++ = 0;
        struct swap *s = &swaps[swap_count];
        s->placeholder = strdup(line);
        s->len = strlen(line);
        snprintf(s->hosts, sizeof s->hosts, "%s", hosts);
        s->ca = ca && *ca ? strdup(ca) : 0;
        if (s->placeholder && s->len) swap_count++;
    }
    free(reply);
}

/* ---- Checking the connection ---- */

static X509_STORE *system_store, *ca_stores[MAX_SWAPS];
static char *ca_paths[MAX_SWAPS];

/* Trusted roots: the system's default bundle and folder (not SSL_CERT_FILE,
 * which whoever starts the program can set), plus the key's own CA if it has one. */
static X509_STORE *store_for(const char *ca) {
    REAL(X509_STORE *, X509_STORE_new, (void));
    REAL(int, X509_STORE_load_locations, (X509_STORE *, const char *, const char *));
    REAL(const char *, X509_get_default_cert_file, (void));
    REAL(const char *, X509_get_default_cert_dir, (void));
    if (!real_X509_STORE_new || !real_X509_STORE_load_locations || !real_X509_get_default_cert_file || !real_X509_get_default_cert_dir) return 0;
    X509_STORE **slot = &system_store;
    if (ca) {
        int i = 0;
        while (i < MAX_SWAPS && ca_paths[i] && strcmp(ca_paths[i], ca)) i++;
        if (i == MAX_SWAPS) return 0;
        if (!ca_paths[i]) ca_paths[i] = strdup(ca);
        slot = &ca_stores[i];
    }
    if (!*slot) {
        X509_STORE *store = real_X509_STORE_new();
        if (!store) return 0;
        real_X509_STORE_load_locations(store, real_X509_get_default_cert_file(), 0);
        real_X509_STORE_load_locations(store, 0, real_X509_get_default_cert_dir());
        if (ca) real_X509_STORE_load_locations(store, ca, 0);
        *slot = store;
    }
    return *slot;
}

/* Whether the peer's chain verifies and its certificate is valid for one of
 * `hosts` (comma-separated); the matching host goes in `matched`. */
static int verified(SSL *ssl, const char *hosts, const char *ca, char *matched, size_t cap) {
    REAL(X509 *, SSL_get1_peer_certificate, (const SSL *));
    REAL(X509 *, SSL_get_peer_certificate, (const SSL *));
    REAL(STACK *, SSL_get_peer_cert_chain, (const SSL *));
    REAL(X509_STORE_CTX *, X509_STORE_CTX_new, (void));
    REAL(int, X509_STORE_CTX_init, (X509_STORE_CTX *, X509_STORE *, X509 *, STACK *));
    REAL(int, X509_verify_cert, (X509_STORE_CTX *));
    REAL(void, X509_STORE_CTX_free, (X509_STORE_CTX *));
    REAL(int, X509_check_host, (X509 *, const char *, size_t, unsigned int, char **));
    REAL(void, X509_free, (X509 *));
    X509 *(*peer_cert)(const SSL *) = real_SSL_get1_peer_certificate ? real_SSL_get1_peer_certificate : real_SSL_get_peer_certificate;
    if (!peer_cert || !real_SSL_get_peer_cert_chain || !real_X509_STORE_CTX_new || !real_X509_STORE_CTX_init || !real_X509_verify_cert ||
        !real_X509_STORE_CTX_free || !real_X509_check_host || !real_X509_free)
        return 0;
    X509 *leaf = peer_cert(ssl);
    if (!leaf) return 0;
    int ok = 0;
    X509_STORE *store = store_for(ca);
    X509_STORE_CTX *ctx = store ? real_X509_STORE_CTX_new() : 0;
    if (ctx && real_X509_STORE_CTX_init(ctx, store, leaf, real_SSL_get_peer_cert_chain(ssl)) == 1 && real_X509_verify_cert(ctx) == 1) {
        char list[MAX_HOSTS];
        snprintf(list, sizeof list, "%s", hosts);
        char *save = 0;
        for (char *host = strtok_r(list, ",", &save); host && !ok; host = strtok_r(0, ",", &save)) {
            if (real_X509_check_host(leaf, host, strlen(host), 0, 0) == 1) {
                snprintf(matched, cap, "%s", host);
                ok = 1;
            }
        }
    }
    if (ctx) real_X509_STORE_CTX_free(ctx);
    real_X509_free(leaf);
    return ok;
}

static int unhex(char c) {
    return c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : -1;
}

/* Ask KeyHandoff for the key behind `s`, for `host`. It must be exactly as
 * long as the placeholder; `key` gets it, and the caller wipes it. */
static int fetch(const struct swap *s, const char *host, char *key) {
    char request[1024], reply[4096];
    snprintf(request, sizeof request, "{\"swap\":{\"token\":\"%s\",\"placeholder\":\"%s\",\"host\":\"%s\"}}\n", token, s->placeholder, host);
    int n = ask(request, reply, sizeof reply);
    int ok = n == (int)(3 + 2 * s->len) && !strncmp(reply, "ok ", 3);
    for (size_t i = 0; ok && i < s->len; i++) {
        int hi = unhex(reply[3 + 2 * i]), lo = unhex(reply[4 + 2 * i]);
        if (hi < 0 || lo < 0) ok = 0;
        else key[i] = (char)(hi << 4 | lo);
    }
    explicit_bzero(reply, sizeof reply);
    return ok;
}

/* The bytes to write: `buf` itself, or a copy with each placeholder for a
 * verified host replaced by its key, which the caller wipes and frees. */
static const void *swapped(SSL *ssl, const void *buf, size_t len, char **copy) {
    *copy = 0;
    if (!active || !buf || !len) return buf;
    pthread_mutex_lock(&lock);
    load_swaps();
    pthread_mutex_unlock(&lock);
    for (int i = 0; i < swap_count; i++) {
        const struct swap *s = &swaps[i];
        if (!memmem(buf, len, s->placeholder, s->len)) continue;
        char host[256];
        if (!verified(ssl, s->hosts, s->ca, host, sizeof host)) continue;
        char *key = malloc(s->len);
        if (!key) continue;
        if (fetch(s, host, key)) {
            if (!*copy) {
                *copy = malloc(len);
                if (!*copy) {
                    explicit_bzero(key, s->len);
                    free(key);
                    return buf;
                }
                memcpy(*copy, buf, len);
            }
            for (char *at = *copy; (at = memmem(at, len - (size_t)(at - *copy), s->placeholder, s->len)); at += s->len)
                memcpy(at, key, s->len);
        }
        explicit_bzero(key, s->len);
        free(key);
    }
    if (*copy) {
        /* A retried write may now come from another buffer. */
        REAL(long, SSL_ctrl, (SSL *, int, long, void *));
        if (real_SSL_ctrl) real_SSL_ctrl(ssl, SSL_CTRL_MODE, SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER, 0);
        return *copy;
    }
    return buf;
}

static void wipe(char *copy, size_t len) {
    if (copy) {
        explicit_bzero(copy, len);
        free(copy);
    }
}

int SSL_write(SSL *ssl, const void *buf, int num) {
    REAL(int, SSL_write, (SSL *, const void *, int));
    if (!real_SSL_write) return -1;
    char *copy;
    const void *out = swapped(ssl, buf, num > 0 ? (size_t)num : 0, &copy);
    int r = real_SSL_write(ssl, out, num);
    wipe(copy, num > 0 ? (size_t)num : 0);
    return r;
}

int SSL_write_ex(SSL *ssl, const void *buf, size_t num, size_t *written) {
    REAL(int, SSL_write_ex, (SSL *, const void *, size_t, size_t *));
    if (!real_SSL_write_ex) return 0;
    char *copy;
    const void *out = swapped(ssl, buf, num, &copy);
    int r = real_SSL_write_ex(ssl, out, num, written);
    wipe(copy, num);
    return r;
}

/* ---- Keeping the hook in programs this one starts ---- */

/* `envp` plus KEYHANDOFF_HOOK and this library in LD_PRELOAD, in a new
 * array the caller frees (with its LD_PRELOAD entry), or `envp` itself. */
static char **with_hook(char *const envp[], char **preload) {
    *preload = 0;
    if (!active) return (char **)envp;
    Dl_info info;
    const char *self = dladdr((void *)hook_init, &info) ? info.dli_fname : 0;
    size_t n = 0;
    int has_hook = 0, preload_at = -1;
    for (; envp && envp[n]; n++) {
        if (!strncmp(envp[n], ENV_NAME "=", sizeof ENV_NAME)) has_hook = 1;
        if (!strncmp(envp[n], "LD_PRELOAD=", 11)) preload_at = (int)n;
    }
    char **out = calloc(n + 3, sizeof *out);
    if (!out) return (char **)envp;
    size_t k = 0;
    for (size_t i = 0; i < n; i++)
        if ((int)i != preload_at) out[k++] = envp[i];
    if (!has_hook) out[k++] = hook_env;
    const char *old = preload_at >= 0 ? envp[preload_at] + 11 : "";
    if (self && !strstr(old, self)) {
        size_t size = 12 + strlen(self) + 1 + strlen(old) + 1;
        if ((*preload = malloc(size))) snprintf(*preload, size, "LD_PRELOAD=%s%s%s", self, *old ? ":" : "", old);
        out[k++] = *preload ? *preload : (char *)envp[preload_at];
    } else if (preload_at >= 0) {
        out[k++] = envp[preload_at];
    }
    out[k] = 0;
    return out;
}

static void done_with(char **env, char *const envp[], char *preload) {
    if (env != (char **)envp) free(env);
    free(preload);
}

int execve(const char *path, char *const argv[], char *const envp[]) {
    REAL(int, execve, (const char *, char *const[], char *const[]));
    char *preload;
    char **env = with_hook(envp, &preload);
    int r = real_execve(path, argv, env);
    done_with(env, envp, preload);
    return r;
}

int execvpe(const char *file, char *const argv[], char *const envp[]) {
    REAL(int, execvpe, (const char *, char *const[], char *const[]));
    char *preload;
    char **env = with_hook(envp, &preload);
    int r = real_execvpe(file, argv, env);
    done_with(env, envp, preload);
    return r;
}

int execv(const char *path, char *const argv[]) {
    return execve(path, argv, environ);
}

int execvp(const char *file, char *const argv[]) {
    return execvpe(file, argv, environ);
}

int posix_spawn(pid_t *pid, const char *path, const posix_spawn_file_actions_t *actions, const posix_spawnattr_t *attr, char *const argv[],
                char *const envp[]) {
    REAL(int, posix_spawn, (pid_t *, const char *, const posix_spawn_file_actions_t *, const posix_spawnattr_t *, char *const[], char *const[]));
    char *preload;
    char **env = with_hook(envp, &preload);
    int r = real_posix_spawn(pid, path, actions, attr, argv, env);
    done_with(env, envp, preload);
    return r;
}

int posix_spawnp(pid_t *pid, const char *file, const posix_spawn_file_actions_t *actions, const posix_spawnattr_t *attr, char *const argv[],
                 char *const envp[]) {
    REAL(int, posix_spawnp, (pid_t *, const char *, const posix_spawn_file_actions_t *, const posix_spawnattr_t *, char *const[], char *const[]));
    char *preload;
    char **env = with_hook(envp, &preload);
    int r = real_posix_spawnp(pid, file, actions, attr, argv, env);
    done_with(env, envp, preload);
    return r;
}

/* system() and popen() spawn inside libc, out of reach, so the environment
 * carries the hook for the moment they start. */
int system(const char *command) {
    REAL(int, system, (const char *));
    if (active) setenv(ENV_NAME, hook_env + sizeof ENV_NAME, 1);
    int r = real_system(command);
    if (active) unsetenv(ENV_NAME);
    return r;
}

FILE *popen(const char *command, const char *type) {
    REAL(FILE *, popen, (const char *, const char *));
    if (active) setenv(ENV_NAME, hook_env + sizeof ENV_NAME, 1);
    FILE *r = real_popen(command, type);
    if (active) unsetenv(ENV_NAME);
    return r;
}
