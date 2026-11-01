# Python httpx (OpenSSL via _ssl), HTTP/1.1, key in a header.
import json, os, ssl, time, httpx
cfg = json.load(open("matrix.json"))
key = os.environ["STRIPE_API_KEY"]
for name in ("api.stripe.test", "evil.test"):
    t = time.perf_counter()
    try:
        transport = httpx.HTTPTransport(verify=ssl.create_default_context(cafile=cfg["ca"]))
        with httpx.Client(transport=transport) as c:
            r = c.post("https://127.0.0.1:%d/v1/charges" % cfg["ports"][name], content=b"amount=100", headers={"Host": name, "Authorization": "Bearer " + key}, extensions={"sni_hostname": name})
        print(name, "ok", r.status_code, "TIME %.3f" % ((time.perf_counter() - t) * 1000))
    except (OSError, httpx.HTTPError) as e:
        print(name, "refused", type(e).__name__)
