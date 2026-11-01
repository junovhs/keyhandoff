# Python over TLS: a WebSocket upgrade with the key in a header, then the key
# again in a (masked, as clients must) message.
import base64, json, os, socket, ssl, time
cfg = json.load(open("matrix.json"))
key = os.environ["STRIPE_API_KEY"]
mode = os.environ.get("WS_PART", "upgrade")
for name in ("api.stripe.test", "evil.test"):
    ctx = ssl.create_default_context(cafile=cfg["ca"])
    t = time.perf_counter()
    try:
        with ctx.wrap_socket(socket.create_connection(("127.0.0.1", cfg["ports"][name])), server_hostname=name) as s:
            auth = "Authorization: Bearer %s\r\n" % key if mode == "upgrade" else ""
            s.sendall(("GET /ws HTTP/1.1\r\nHost: %s\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n%s\r\n"
                       % (name, base64.b64encode(os.urandom(16)).decode(), auth)).encode())
            s.recv(1024)
            payload = ("auth " + key).encode() if mode == "message" else b"hello"
            mask = os.urandom(4)
            s.sendall(bytes([0x81, 0x80 | len(payload)]) + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))
            s.recv(10)
        print(name, "ok", "TIME %.3f" % ((time.perf_counter() - t) * 1000))
    except OSError as e:
        print(name, "refused", type(e).__name__)
