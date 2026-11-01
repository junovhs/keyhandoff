# A keep-alive HTTPS server for the benchmarks: answers every request with a
# tiny 200 and discards bodies. Prints its port.  python3 server.py CERT KEY
import socket, ssl, sys, threading
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(sys.argv[1], sys.argv[2])
s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind(("127.0.0.1", 0))
s.listen(16)
print(s.getsockname()[1], flush=True)

def serve(c):
    try:
        t = ctx.wrap_socket(c, server_side=True)
        t.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        f = t.makefile("rb")
        while True:
            length = 0
            while True:
                line = f.readline()
                if not line:
                    return
                if line in (b"\r\n", b"\n"):
                    break
                if line.lower().startswith(b"content-length:"):
                    length = int(line.split(b":")[1])
            while length:
                length -= len(f.read(min(length, 1 << 20)))
            t.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
    except (OSError, ValueError):
        pass

while True:
    c, _ = s.accept()
    threading.Thread(target=serve, args=(c,), daemon=True).start()
