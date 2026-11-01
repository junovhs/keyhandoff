/*
 * KeyHandoff's TLS hook, preloaded by `keyhandoff run` into the programs it
 * starts (LD_PRELOAD). The program holds a placeholder that looks like the
 * key. When an outgoing TLS write contains one, this checks the connection's
 * certificate itself, against the system's trusted roots rather than the
 * program's (which code in the program could change), and only when the
 * certificate is valid for one of the key's hosts asks KeyHandoff for the
 * real key, swaps it into a copy of the bytes being encrypted, writes them
 * and wipes the copy.
 *
 * A placeholder going anywhere else is a leak in the act, since nothing
 * legitimate sends it elsewhere: the write is refused before a byte leaves,
 * over TLS or a plain socket, and KeyHandoff is told where it was going. It
 * remembers that destination, and the hook refuses connections to it, and
 * to those in teammates' warnings, from then on.
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
#include <arpa/inet.h>
#include <netdb.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <sys/uio.h>
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
#define MAX_BLOCKED 128
#define TLSEXT_NAMETYPE_host_name 0
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
static char *blocked[MAX_BLOCKED];
static int blocked_count;
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
/* Set while this hook talks to KeyHandoff, so its own socket calls pass. */
static __thread int talking;

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
    talking++;
    int connected = connect(fd, (struct sockaddr *)&addr, sizeof addr);
    talking--;
    if (connected < 0) {
        close(fd);
        return -1;
    }
    size_t sent = 0, n = strlen(line);
    while (sent < n) {
        talking++;
        ssize_t w = send(fd, line + sent, n - sent, MSG_NOSIGNAL);
        talking--;
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

/* The run's placeholders, asked for once: "placeholder\thost,host\tca" per
 * line, and destinations to refuse, "!destination" per line. */
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
    char *save = 0;
    for (char *line = strtok_r(reply, "\n", &save); line && swap_count < MAX_SWAPS; line = strtok_r(0, "\n", &save)) {
        if (*line == '!') {
            if (blocked_count < MAX_BLOCKED && line[1]) blocked[blocked_count++] = strdup(line + 1);
            continue;
        }
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

static void ensure_loaded(void) {
    if (swap_count >= 0) return;
    pthread_mutex_lock(&lock);
    load_swaps();
    pthread_mutex_unlock(&lock);
}

static int is_blocked(const char *destination) {
    for (int i = 0; destination && *destination && i < blocked_count; i++)
        if (blocked[i] && !strcasecmp(blocked[i], destination)) return 1;
    return 0;
}

/* Say so on the program's stderr, past this hook. */
static void say(const char *what, const char *destination) {
    REAL(ssize_t, write, (int, const void *, size_t));
    char line[400];
    int n = snprintf(line, sizeof line, "KeyHandoff %s %s.\n", what, destination);
    if (real_write && n > 0) real_write(2, line, (size_t)n < sizeof line ? (size_t)n : sizeof line - 1);
}

/* Tell KeyHandoff a placeholder was stopped on its way to `destination`. */
static void report(const struct swap *s, const char *destination) {
    char safe[256], request[1024], reply[256];
    size_t k = 0;
    for (const char *c = destination; *c && k + 1 < sizeof safe; c++)
        if ((*c >= 'a' && *c <= 'z') || (*c >= 'A' && *c <= 'Z') || (*c >= '0' && *c <= '9') || strchr(".-:[]_", *c)) safe[k++] = *c;
    safe[k] = 0;
    snprintf(request, sizeof request, "{\"alert\":{\"token\":\"%s\",\"placeholder\":\"%s\",\"destination\":\"%s\"}}\n", token, s->placeholder, safe);
    ask(request, reply, sizeof reply);
    say("stopped a key's stand-in going to", destination);
}

/* The first placeholder in `buf`, if any. */
static const struct swap *placeholder_in(const void *buf, size_t len) {
    for (int i = 0; buf && i < swap_count; i++)
        if (memmem(buf, len, swaps[i].placeholder, swaps[i].len)) return &swaps[i];
    return 0;
}

/* "1.2.3.4:443" or "[::1]:443" for an internet address; empty otherwise. */
static void name_address(const struct sockaddr *sa, char *out, size_t cap) {
    char ip[INET6_ADDRSTRLEN] = "";
    *out = 0;
    if (sa && sa->sa_family == AF_INET) {
        const struct sockaddr_in *in = (const struct sockaddr_in *)sa;
        inet_ntop(AF_INET, &in->sin_addr, ip, sizeof ip);
        snprintf(out, cap, "%s:%u", ip, ntohs(in->sin_port));
    } else if (sa && sa->sa_family == AF_INET6) {
        const struct sockaddr_in6 *in6 = (const struct sockaddr_in6 *)sa;
        inet_ntop(AF_INET6, &in6->sin6_addr, ip, sizeof ip);
        snprintf(out, cap, "[%s]:%u", ip, ntohs(in6->sin6_port));
    }
}

/* Where an internet socket is connected; empty for anything else. */
static void peer_of(int fd, char *out, size_t cap) {
    struct sockaddr_storage ss;
    socklen_t len = sizeof ss;
    *out = 0;
    if (getpeername(fd, (struct sockaddr *)&ss, &len) == 0) name_address((struct sockaddr *)&ss, out, cap);
}

/* Whether an address, or its bare IP, is a refused destination. */
static int address_blocked(const char *address) {
    if (!*address || !blocked_count) return 0;
    if (is_blocked(address)) return 1;
    char ip[INET6_ADDRSTRLEN + 2];
    snprintf(ip, sizeof ip, "%s", address);
    char *colon = strrchr(ip, ':');
    if (colon) *colon = 0;
    char *bare = ip[0] == '[' ? ip + 1 : ip;
    char *close = strchr(bare, ']');
    if (close) *close = 0;
    return is_blocked(bare);
}

static void wipe(char *copy, size_t len);

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

/* The name a TLS connection was opened for (SNI), or its peer's address. */
static void destination_of(SSL *ssl, char *out, size_t cap) {
    REAL(const char *, SSL_get_servername, (const SSL *, int));
    REAL(int, SSL_get_fd, (const SSL *));
    const char *name = real_SSL_get_servername ? real_SSL_get_servername(ssl, TLSEXT_NAMETYPE_host_name) : 0;
    if (name && *name) {
        snprintf(out, cap, "%s", name);
        return;
    }
    *out = 0;
    if (real_SSL_get_fd) peer_of(real_SSL_get_fd(ssl), out, cap);
    if (!*out) snprintf(out, cap, "an unknown server");
}

/* Whether this TLS connection is to a refused destination: by the name it
 * was opened for, a name its certificate is valid for, or its address. */
static int tls_blocked(SSL *ssl, const char *destination) {
    if (!blocked_count) return 0;
    if (is_blocked(destination)) return 1;
    REAL(int, SSL_get_fd, (const SSL *));
    char address[80] = "";
    if (real_SSL_get_fd) peer_of(real_SSL_get_fd(ssl), address, sizeof address);
    if (address_blocked(address)) return 1;
    REAL(X509 *, SSL_get1_peer_certificate, (const SSL *));
    REAL(X509 *, SSL_get_peer_certificate, (const SSL *));
    REAL(int, X509_check_host, (X509 *, const char *, size_t, unsigned int, char **));
    REAL(void, X509_free, (X509 *));
    X509 *(*peer_cert)(const SSL *) = real_SSL_get1_peer_certificate ? real_SSL_get1_peer_certificate : real_SSL_get_peer_certificate;
    X509 *leaf = peer_cert ? peer_cert(ssl) : 0;
    int hit = 0;
    for (int i = 0; leaf && real_X509_check_host && i < blocked_count && !hit; i++)
        hit = blocked[i] && real_X509_check_host(leaf, blocked[i], strlen(blocked[i]), 0, 0) == 1;
    if (leaf && real_X509_free) real_X509_free(leaf);
    return hit;
}

/* The bytes to write: `buf` itself, or a copy with each placeholder for a
 * verified host replaced by its key, which the caller wipes and frees; or
 * null to refuse the write, when a placeholder would go anywhere else or the
 * connection is to a refused destination. */
static const void *swapped(SSL *ssl, const void *buf, size_t len, char **copy) {
    *copy = 0;
    if (!active || !buf || !len) return buf;
    ensure_loaded();
    char destination[256];
    destination_of(ssl, destination, sizeof destination);
    const struct swap *leaking = 0;
    for (int i = 0; i < swap_count; i++) {
        const struct swap *s = &swaps[i];
        if (!memmem(buf, len, s->placeholder, s->len)) continue;
        char host[256];
        if (tls_blocked(ssl, destination) || !verified(ssl, s->hosts, s->ca, host, sizeof host)) {
            leaking = s;
            break;
        }
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
    if (leaking) {
        wipe(*copy, len);
        *copy = 0;
        report(leaking, destination);
        return 0;
    }
    if (tls_blocked(ssl, destination)) {
        wipe(*copy, len);
        *copy = 0;
        say("refused a connection to", destination);
        return 0;
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
    if (!out) {
        errno = ECONNREFUSED;
        return -1;
    }
    int r = real_SSL_write(ssl, out, num);
    wipe(copy, num > 0 ? (size_t)num : 0);
    return r;
}

int SSL_write_ex(SSL *ssl, const void *buf, size_t num, size_t *written) {
    REAL(int, SSL_write_ex, (SSL *, const void *, size_t, size_t *));
    if (!real_SSL_write_ex) return 0;
    char *copy;
    const void *out = swapped(ssl, buf, num, &copy);
    if (!out) {
        if (written) *written = 0;
        errno = ECONNREFUSED;
        return 0;
    }
    int r = real_SSL_write_ex(ssl, out, num, written);
    wipe(copy, num);
    return r;
}

/* ---- Plain sockets ---- */

/* Whether a write on `fd` (to `to`, if given) must be refused: a placeholder
 * in clear text to an internet address. */
static int plain_leak(int fd, const struct sockaddr *to, const void *buf, size_t len) {
    if (!active || talking || !buf || !len) return 0;
    char destination[80] = "";
    int named = 0;
    if (swap_count < 0) {
        /* Not loaded yet: only an internet socket is worth asking KeyHandoff about. */
        if (to) name_address(to, destination, sizeof destination);
        else peer_of(fd, destination, sizeof destination);
        if (!*destination) return 0;
        named = 1;
        ensure_loaded();
    }
    const struct swap *s = placeholder_in(buf, len);
    if (!s) return 0;
    if (!named) {
        if (to) name_address(to, destination, sizeof destination);
        else peer_of(fd, destination, sizeof destination);
    }
    if (!*destination) return 0; /* a pipe, a file, a local socket */
    report(s, destination);
    errno = ECONNREFUSED;
    return 1;
}

ssize_t send(int fd, const void *buf, size_t len, int flags) {
    REAL(ssize_t, send, (int, const void *, size_t, int));
    if (plain_leak(fd, 0, buf, len)) return -1;
    return real_send(fd, buf, len, flags);
}

ssize_t sendto(int fd, const void *buf, size_t len, int flags, const struct sockaddr *to, socklen_t tolen) {
    REAL(ssize_t, sendto, (int, const void *, size_t, int, const struct sockaddr *, socklen_t));
    if (plain_leak(fd, to, buf, len)) return -1;
    return real_sendto(fd, buf, len, flags, to, tolen);
}

ssize_t write(int fd, const void *buf, size_t len) {
    REAL(ssize_t, write, (int, const void *, size_t));
    if (plain_leak(fd, 0, buf, len)) return -1;
    return real_write(fd, buf, len);
}

static int iov_leak(int fd, const struct sockaddr *to, const struct iovec *iov, int count) {
    for (int i = 0; iov && i < count; i++)
        if (plain_leak(fd, to, iov[i].iov_base, iov[i].iov_len)) return 1;
    return 0;
}

ssize_t writev(int fd, const struct iovec *iov, int count) {
    REAL(ssize_t, writev, (int, const struct iovec *, int));
    if (iov_leak(fd, 0, iov, count)) return -1;
    return real_writev(fd, iov, count);
}

ssize_t sendmsg(int fd, const struct msghdr *msg, int flags) {
    REAL(ssize_t, sendmsg, (int, const struct msghdr *, int));
    if (msg && iov_leak(fd, (const struct sockaddr *)msg->msg_name, msg->msg_iov, (int)msg->msg_iovlen)) return -1;
    return real_sendmsg(fd, msg, flags);
}

/* Refused destinations: connections to their addresses, and looking up their names. */
int connect(int fd, const struct sockaddr *to, socklen_t len) {
    REAL(int, connect, (int, const struct sockaddr *, socklen_t));
    if (active && !talking && to && (to->sa_family == AF_INET || to->sa_family == AF_INET6)) {
        ensure_loaded();
        char destination[80];
        name_address(to, destination, sizeof destination);
        if (address_blocked(destination)) {
            say("refused a connection to", destination);
            errno = ECONNREFUSED;
            return -1;
        }
    }
    return real_connect(fd, to, len);
}

int getaddrinfo(const char *node, const char *service, const struct addrinfo *hints, struct addrinfo **res) {
    REAL(int, getaddrinfo, (const char *, const char *, const struct addrinfo *, struct addrinfo **));
    if (active && !talking && node) {
        ensure_loaded();
        if (is_blocked(node)) {
            say("refused a connection to", node);
            return EAI_NONAME;
        }
    }
    return real_getaddrinfo(node, service, hints, res);
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
