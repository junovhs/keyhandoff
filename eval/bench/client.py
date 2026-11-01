# The benchmark client: over one keep-alive TLS connection to the key's
# server, time N small requests carrying STRIPE_API_KEY in a header ("keyed")
# or no key ("plain"), then upload MB megabytes in 64 KiB writes ("bulk").
# Prints one JSON line.  python3 client.py MODE N MB
import json, os, socket, ssl, statistics, sys, time
cfg = json.load(open("bench.json"))
mode, n, mb = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
key = os.environ.get("STRIPE_API_KEY", "")
ctx = ssl.create_default_context(cafile=cfg["ca"])
s = ctx.wrap_socket(socket.create_connection(("127.0.0.1", cfg["port"])), server_hostname="api.stripe.test")
s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
f = s.makefile("rb")
auth = ("Authorization: Bearer %s\r\n" % key) if mode == "keyed" else "X-Note: none\r\n"
req = ("GET /v1/balance HTTP/1.1\r\nHost: api.stripe.test\r\n%s\r\n" % auth).encode()

def one():
    s.sendall(req)
    while f.readline() not in (b"\r\n", b"\n"):
        pass
    f.read(2)

for _ in range(50):
    one()
times = []
for _ in range(n):
    t = time.perf_counter()
    one()
    times.append((time.perf_counter() - t) * 1e6)
chunk = b"x" * 65536
total = mb * 1024 * 1024
s.sendall(b"POST /upload HTTP/1.1\r\nHost: api.stripe.test\r\nContent-Length: %d\r\n\r\n" % total)
t = time.perf_counter()
for _ in range(total // len(chunk)):
    s.sendall(chunk)
while f.readline() not in (b"\r\n", b"\n"):
    pass
f.read(2)
secs = time.perf_counter() - t
times.sort()
print(json.dumps({
    "mode": mode, "requests": n,
    "median_us": statistics.median(times), "p95_us": times[int(0.95 * n)],
    "mean_us": statistics.fmean(times), "stdev_us": statistics.stdev(times),
    "upload_mb_s": mb / secs,
}))
