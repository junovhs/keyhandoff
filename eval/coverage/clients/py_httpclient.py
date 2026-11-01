# Python http.client (OpenSSL via _ssl), HTTP/1.1, key in a header.
import http.client, json, os, ssl, time
cfg = json.load(open("matrix.json"))
key = os.environ["STRIPE_API_KEY"]
for name in ("api.stripe.test", "evil.test"):
    ctx = ssl.create_default_context(cafile=cfg["ca"])
    t = time.perf_counter()
    try:
        c = http.client.HTTPSConnection("127.0.0.1", cfg["ports"][name], context=ctx)
        c._tunnel_host = None
        c.sock = ctx.wrap_socket(__import__("socket").create_connection(("127.0.0.1", cfg["ports"][name])), server_hostname=name)
        c.request("POST", "/v1/charges", body="amount=100", headers={"Host": name, "Authorization": "Bearer " + key})
        c.getresponse().read()
        print(name, "ok", "TIME %.3f" % ((time.perf_counter() - t) * 1000))
    except OSError as e:
        print(name, "refused", type(e).__name__)
