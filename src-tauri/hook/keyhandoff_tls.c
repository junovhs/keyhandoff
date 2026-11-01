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

static void hp_init(void);

__attribute__((constructor)) static void hook_init(void) {
    const char *value = getenv(ENV_NAME);
    if (!value || strlen(value) < TOKEN_HEX + 2 || value[TOKEN_HEX] != ':' || strlen(value + TOKEN_HEX + 1) >= sizeof socket_path)
        return;
    memcpy(token, value, TOKEN_HEX);
    strcpy(socket_path, value + TOKEN_HEX + 1);
    snprintf(hook_env, sizeof hook_env, "%s=%s", ENV_NAME, value);
    unsetenv(ENV_NAME);
    hp_init();
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

/* ---- HTTP/2 ----
 * HTTP/2 compresses headers (HPACK) before they reach SSL_write, usually
 * with a Huffman code, so a placeholder in a header isn't there as written.
 * On a connection that chose h2, the hook follows the frames from the
 * connection preface on, decodes each complete header block's string
 * literals, swaps placeholders where the connection allows, and encodes
 * the string again the same way (same representation, same table effect:
 * the key is as long as its placeholder, so table sizes don't change). A
 * header block split over CONTINUATION frames or writes isn't rewritten. */

#include "hpack_huffman.h"

#define H2_PREFACE "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
#define H2_CONNS 128

static unsigned short hp_sorted[257];
static unsigned int hp_first[31];
static int hp_count[31], hp_index[31];

/* Canonical decoding tables: symbols in code order, and where each length starts. */
static void hp_init(void) {
    int k = 0;
    for (int len = 1; len <= 30; len++) {
        hp_index[len] = k;
        for (int sym = 0; sym < 257; sym++)
            if (hpack_lengths[sym] == len) hp_sorted[k++] = (unsigned short)sym;
        hp_count[len] = k - hp_index[len];
        hp_first[len] = hp_count[len] ? hpack_codes[hp_sorted[hp_index[len]]] : 0;
    }
}

static long hp_decode(const unsigned char *in, size_t n, unsigned char *out, size_t cap) {
    unsigned int code = 0;
    int len = 0;
    size_t o = 0;
    for (size_t i = 0; i < n; i++) {
        for (int bit = 7; bit >= 0; bit--) {
            code = code << 1 | ((in[i] >> bit) & 1);
            if (++len > 30) return -1;
            if (hp_count[len] && code >= hp_first[len] && code - hp_first[len] < (unsigned)hp_count[len]) {
                int sym = hp_sorted[hp_index[len] + (int)(code - hp_first[len])];
                if (sym == 256 || o >= cap) return -1;
                out[o++] = (unsigned char)sym;
                code = 0;
                len = 0;
            }
        }
    }
    /* What's left is padding: under 8 bits, all ones. */
    if (len > 7 || code != (1u << len) - 1) return -1;
    return (long)o;
}

static size_t hp_encode(const unsigned char *in, size_t n, unsigned char *out) {
    unsigned long long bits = 0;
    int held = 0;
    size_t o = 0;
    for (size_t i = 0; i < n; i++) {
        bits = bits << hpack_lengths[in[i]] | hpack_codes[in[i]];
        held += hpack_lengths[in[i]];
        while (held >= 8) {
            held -= 8;
            out[o++] = (unsigned char)(bits >> held);
        }
        bits &= (1ULL << held) - 1;
    }
    if (held) out[o++] = (unsigned char)(bits << (8 - held) | ((1u << (8 - held)) - 1));
    return o;
}

static int hp_int(const unsigned char *p, size_t n, size_t *i, int prefix, size_t *out) {
    if (*i >= n) return 0;
    size_t max = (1u << prefix) - 1, v = p[(*i)++] & max;
    if (v < max) {
        *out = v;
        return 1;
    }
    for (int shift = 0; *i < n && shift <= 21; shift += 7) {
        unsigned char b = p[(*i)++];
        v += (size_t)(b & 0x7f) << shift;
        if (!(b & 0x80)) {
            *out = v;
            return 1;
        }
    }
    return 0;
}

static size_t hp_put_int(unsigned char *o, unsigned char high, int prefix, size_t v) {
    size_t max = (1u << prefix) - 1, k = 1;
    if (v < max) {
        o[0] = (unsigned char)(high | v);
        return 1;
    }
    o[0] = (unsigned char)(high | max);
    for (v -= max; v >= 128; v >>= 7) o[k++] = (unsigned char)(v & 0x7f) | 0x80;
    o[k++] = (unsigned char)v;
    return k;
}

/* Where each connection is in its frames: bytes of a frame still to come. */
struct h2conn {
    SSL *ssl;
    size_t skip;
};
static struct h2conn h2conns[H2_CONNS];
static pthread_mutex_t h2lock = PTHREAD_MUTEX_INITIALIZER;

/* What a write does to that, applied once the write goes through. */
struct h2plan {
    int track; /* 1: from the preface, 2: carry on, 0: nothing to do */
    int lost;  /* stop following this connection */
    size_t skip;
};

static struct h2conn *h2_find(SSL *ssl) {
    for (int i = 0; i < H2_CONNS; i++)
        if (h2conns[i].ssl == ssl) return &h2conns[i];
    return 0;
}

static void h2_commit(SSL *ssl, const struct h2plan *plan) {
    if (!plan->track) return;
    pthread_mutex_lock(&h2lock);
    struct h2conn *c = h2_find(ssl);
    if (!c && plan->track == 1) c = h2_find(0);
    if (!c && plan->track == 1) c = &h2conns[(size_t)ssl / 64 % H2_CONNS];
    if (c) {
        c->ssl = plan->lost ? 0 : ssl;
        c->skip = plan->skip;
    }
    pthread_mutex_unlock(&h2lock);
}

static int is_h2(SSL *ssl) {
    REAL(void, SSL_get0_alpn_selected, (const SSL *, const unsigned char **, unsigned int *));
    const unsigned char *alpn = 0;
    unsigned int n = 0;
    if (real_SSL_get0_alpn_selected) real_SSL_get0_alpn_selected(ssl, &alpn, &n);
    return n == 2 && alpn && !memcmp(alpn, "h2", 2);
}

/* What this write does with each placeholder on this connection. */
enum { UNDECIDED, SWAP, ELSEWHERE, NO_KEY };
struct verdict {
    int state;
    char *key;
};

static int decide(SSL *ssl, const char *destination, int i, struct verdict *v) {
    if (v[i].state == UNDECIDED) {
        const struct swap *s = &swaps[i];
        char host[256];
        v[i].state = ELSEWHERE;
        if (!tls_blocked(ssl, destination) && verified(ssl, s->hosts, s->ca, host, sizeof host)) {
            v[i].state = NO_KEY; /* the right server, but the run is over: the placeholder goes */
            v[i].key = malloc(s->len);
            if (v[i].key && fetch(s, host, v[i].key)) v[i].state = SWAP;
        }
    }
    return v[i].state;
}

/* Swap placeholders in `text` (decoded header value or body) where allowed.
 * Returns -1 if one would go elsewhere (its index in *leak), else whether
 * anything changed. */
static int swap_in(SSL *ssl, const char *destination, unsigned char *text, size_t n, struct verdict *v, int *leak) {
    int changed = 0;
    for (int i = 0; i < swap_count; i++) {
        const struct swap *s = &swaps[i];
        if (!memmem(text, n, s->placeholder, s->len)) continue;
        int state = decide(ssl, destination, i, v);
        if (state == ELSEWHERE) {
            *leak = i;
            return -1;
        }
        if (state != SWAP) continue;
        for (unsigned char *at = text; (at = memmem(at, n - (size_t)(at - text), s->placeholder, s->len)); at += s->len) memcpy(at, v[i].key, s->len);
        changed = 1;
    }
    return changed;
}

/* A header block with its value strings swapped, in a new buffer, or null
 * when nothing changed or it can't be read. */
static unsigned char *h2_block(SSL *ssl, const char *destination, const unsigned char *b, size_t n, size_t *out_n, struct verdict *v, int *leak) {
    unsigned char *out = 0, *text = 0;
    size_t o = 0, copied = 0, i = 0;
    while (i < n) {
        size_t value;
        unsigned char first = b[i];
        if (first & 0x80) { /* indexed */
            if (!hp_int(b, n, &i, 7, &value)) goto unreadable;
            continue;
        }
        if ((first & 0xe0) == 0x20) { /* table size update */
            if (!hp_int(b, n, &i, 5, &value)) goto unreadable;
            continue;
        }
        size_t name;
        if (!hp_int(b, n, &i, (first & 0xc0) == 0x40 ? 6 : 4, &name)) goto unreadable;
        for (int part = name ? 1 : 0; part < 2; part++) { /* a literal name, then the value */
            size_t at = i, len;
            int huffman = i < n && (b[i] & 0x80);
            if (!hp_int(b, n, &i, 7, &len) || len > n - i) goto unreadable;
            if (part == 0) {
                i += len;
                continue;
            }
            size_t cap = len * 8 / 5 + 1;
            text = malloc(cap);
            if (!text) goto unreadable;
            long tn = huffman ? hp_decode(b + i, len, text, cap) : (long)len;
            if (!huffman) memcpy(text, b + i, len);
            i += len;
            if (tn < 0) goto unreadable;
            int changed = swap_in(ssl, destination, text, (size_t)tn, v, leak);
            if (changed < 0) goto unreadable;
            if (changed) {
                if (!out && !(out = malloc(n * 4 + 64))) goto unreadable;
                memcpy(out + o, b + copied, at - copied);
                o += at - copied;
                if (huffman) {
                    unsigned char *enc = malloc((size_t)tn * 4 + 8);
                    if (!enc) goto unreadable;
                    size_t en = hp_encode(text, (size_t)tn, enc);
                    o += hp_put_int(out + o, 0x80, 7, en);
                    memcpy(out + o, enc, en);
                    o += en;
                    explicit_bzero(enc, en);
                    free(enc);
                } else {
                    o += hp_put_int(out + o, 0, 7, (size_t)tn);
                    memcpy(out + o, text, (size_t)tn);
                    o += (size_t)tn;
                }
                copied = i;
            }
            explicit_bzero(text, cap);
            free(text);
            text = 0;
        }
    }
    if (!out) return 0;
    memcpy(out + o, b + copied, n - copied);
    *out_n = o + n - copied;
    return out;
unreadable:
    if (text) {
        explicit_bzero(text, 1);
        free(text);
    }
    if (out) {
        explicit_bzero(out, o);
        free(out);
    }
    return 0;
}

/* The write with its HTTP/2 header blocks swapped, in a new buffer, or null
 * when nothing changed (or this isn't an HTTP/2 connection being followed). */
static unsigned char *h2_rewrite(SSL *ssl, const char *destination, const unsigned char *b, size_t n, size_t *out_n, struct verdict *v, int *leak,
                                 struct h2plan *plan) {
    size_t pos = 0;
    plan->track = 0;
    if (n >= 24 && !memcmp(b, H2_PREFACE, 24)) {
        plan->track = 1;
        pos = 24;
    } else {
        pthread_mutex_lock(&h2lock);
        struct h2conn *c = h2_find(ssl);
        size_t skip = c ? c->skip : 0;
        pthread_mutex_unlock(&h2lock);
        if (!c) return 0;
        plan->track = 2;
        if (skip >= n) {
            plan->skip = skip - n;
            return 0;
        }
        pos = skip;
    }
    plan->skip = 0;
    plan->lost = 0;
    unsigned char *out = 0;
    size_t o = 0, copied = 0;
    while (pos < n) {
        if (n - pos < 9) {
            plan->lost = 1; /* a frame header split between writes */
            break;
        }
        size_t len = (size_t)b[pos] << 16 | (size_t)b[pos + 1] << 8 | b[pos + 2], end = pos + 9 + len;
        unsigned char type = b[pos + 3], flags = b[pos + 4];
        if (end > n) {
            plan->skip = end - n;
            break;
        }
        if (type == 1 && (flags & 0x4)) { /* HEADERS with END_HEADERS */
            size_t p = pos + 9, pad = 0;
            if (flags & 0x8) pad = b[p++];
            if (flags & 0x20) p += 5;
            if (p + pad <= end) {
                size_t block_n = 0;
                unsigned char *block = h2_block(ssl, destination, b + p, end - pad - p, &block_n, v, leak);
                if (*leak >= 0) break;
                size_t frame = (p - pos - 9) + block_n + pad;
                if (block && frame <= 0xffffff) {
                    if (!out && !(out = malloc(n * 4 + 64))) break;
                    memcpy(out + o, b + copied, pos - copied);
                    o += pos - copied;
                    out[o++] = (unsigned char)(frame >> 16);
                    out[o++] = (unsigned char)(frame >> 8);
                    out[o++] = (unsigned char)frame;
                    memcpy(out + o, b + pos + 3, p - pos - 3);
                    o += p - pos - 3;
                    memcpy(out + o, block, block_n);
                    o += block_n;
                    memcpy(out + o, b + end - pad, pad);
                    o += pad;
                    copied = end;
                }
                if (block) {
                    explicit_bzero(block, block_n);
                    free(block);
                }
            }
        }
        pos = end;
    }
    if (!out) return 0;
    if (*leak >= 0) {
        explicit_bzero(out, o);
        free(out);
        return 0;
    }
    memcpy(out + o, b + copied, n - copied);
    *out_n = o + n - copied;
    return out;
}

/* ---- The write ---- */

/* The bytes to write: `buf` itself, or a copy (in *copy, *out_n bytes, for
 * the caller to wipe and free) with placeholders swapped where the
 * connection is verified for their key; or null to refuse the write, when a
 * placeholder would go anywhere else or the connection is to a refused
 * destination. */
static const void *swapped(SSL *ssl, const void *buf, size_t len, char **copy, size_t *out_n, struct h2plan *plan) {
    *copy = 0;
    *out_n = len;
    plan->track = 0;
    if (!active || !buf || !len) return buf;
    ensure_loaded();
    char destination[256];
    destination_of(ssl, destination, sizeof destination);
    struct verdict v[MAX_SWAPS] = {{0}};
    int leak = -1;
    unsigned char *work = (unsigned char *)buf;
    size_t n = len;
    if (is_h2(ssl)) {
        size_t h2n;
        unsigned char *h2 = h2_rewrite(ssl, destination, work, n, &h2n, v, &leak, plan);
        if (h2) {
            work = h2;
            n = h2n;
            *copy = (char *)h2;
        }
    }
    if (leak < 0) {
        /* Placeholders as written: HTTP/1, bodies, uncompressed headers. */
        int changed = 0;
        for (int i = 0; i < swap_count && leak < 0; i++) {
            const struct swap *s = &swaps[i];
            if (!memmem(work, n, s->placeholder, s->len)) continue;
            int state = decide(ssl, destination, i, v);
            if (state == ELSEWHERE) leak = i;
            if (state != SWAP) continue;
            if (!*copy) {
                if (!(*copy = malloc(n))) break;
                memcpy(*copy, work, n);
                work = (unsigned char *)*copy;
            }
            for (unsigned char *at = work; (at = memmem(at, n - (size_t)(at - work), s->placeholder, s->len)); at += s->len) memcpy(at, v[i].key, s->len);
            changed = 1;
        }
        (void)changed;
    }
    for (int i = 0; i < swap_count; i++) {
        if (v[i].key) {
            explicit_bzero(v[i].key, swaps[i].len);
            free(v[i].key);
        }
    }
    if (leak >= 0) {
        wipe(*copy, n);
        *copy = 0;
        plan->track = 0;
        report(&swaps[leak], destination);
        return 0;
    }
    if (tls_blocked(ssl, destination)) {
        wipe(*copy, n);
        *copy = 0;
        plan->track = 0;
        say("refused a connection to", destination);
        return 0;
    }
    if (*copy) {
        /* A retried write may now come from another buffer. */
        REAL(long, SSL_ctrl, (SSL *, int, long, void *));
        if (real_SSL_ctrl) real_SSL_ctrl(ssl, SSL_CTRL_MODE, SSL_MODE_ACCEPT_MOVING_WRITE_BUFFER, 0);
        *out_n = n;
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
    size_t n;
    struct h2plan plan;
    const void *out = swapped(ssl, buf, num > 0 ? (size_t)num : 0, &copy, &n, &plan);
    if (!out) {
        errno = ECONNREFUSED;
        return -1;
    }
    int r = real_SSL_write(ssl, out, (int)n);
    wipe(copy, n);
    if (r > 0) h2_commit(ssl, &plan);
    /* A rewritten HTTP/2 header can change the length: all of it went, or none. */
    if (r > 0 && n != (size_t)num) return r == (int)n ? num : -1;
    return r;
}

int SSL_write_ex(SSL *ssl, const void *buf, size_t num, size_t *written) {
    REAL(int, SSL_write_ex, (SSL *, const void *, size_t, size_t *));
    if (!real_SSL_write_ex) return 0;
    char *copy;
    size_t n, sent = 0;
    struct h2plan plan;
    const void *out = swapped(ssl, buf, num, &copy, &n, &plan);
    if (!out) {
        if (written) *written = 0;
        errno = ECONNREFUSED;
        return 0;
    }
    int r = real_SSL_write_ex(ssl, out, n, &sent);
    wipe(copy, n);
    if (r == 1) h2_commit(ssl, &plan);
    if (written) *written = r == 1 && n != num ? (sent == n ? num : 0) : sent;
    return r == 1 && n != num && sent != n ? 0 : r;
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
