# Python requests/urllib3 (OpenSSL via _ssl), HTTP/1.1, key in a header.
import json, os, time, requests
from requests.adapters import HTTPAdapter
cfg = json.load(open("matrix.json"))
key = os.environ["STRIPE_API_KEY"]
for name in ("api.stripe.test", "evil.test"):
    s = requests.Session()
    t = time.perf_counter()
    try:
        # Resolve the name to the local server, keeping SNI and verification on the name.
        import urllib3.util.connection as conn
        real = conn.create_connection
        conn.create_connection = lambda addr, *a, **k: real(("127.0.0.1", cfg["ports"][name]), *a, **k)
        r = s.post("https://%s:%d/v1/charges" % (name, cfg["ports"][name]), data="amount=100", headers={"Authorization": "Bearer " + key}, verify=cfg["ca"], timeout=10)
        conn.create_connection = real
        print(name, "ok", r.status_code, "TIME %.3f" % ((time.perf_counter() - t) * 1000))
    except (OSError, requests.RequestException) as e:
        print(name, "refused", type(e).__name__)
