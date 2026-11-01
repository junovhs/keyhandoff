// A test server for the coverage matrix: HTTP/1.1, HTTP/2 and WebSocket over
// TLS on one port (or plain HTTP with --plain), recording every request it
// gets as one JSON line: protocol, headers, body (gunzipped when gzip), and
// WebSocket messages. Prints its port, then serves until killed.
//
//   node server.mjs NAME CERT KEY OUT [--plain]
import { createHash } from "node:crypto";
import { appendFileSync, readFileSync } from "node:fs";
import { createServer as plainServer } from "node:http";
import { createSecureServer } from "node:http2";
import { gunzipSync } from "node:zlib";

const [name, cert, key, out, plain] = process.argv.slice(2);
const record = (entry) => appendFileSync(out, JSON.stringify({ server: name, ...entry }) + "\n");

function handle(req, res) {
  const chunks = [];
  req.on("data", (c) => chunks.push(c));
  req.on("end", () => {
    let body = Buffer.concat(chunks);
    if (req.headers["content-encoding"] === "gzip") {
      try {
        body = gunzipSync(body);
      } catch {}
    }
    record({ proto: req.httpVersion === "2.0" ? "h2" : "h1", headers: req.headers, body: body.toString("latin1") });
    res.writeHead(200, { "content-type": "text/plain" });
    res.end("ok");
  });
}

// WebSocket, just enough to read the client's first message.
function upgrade(req, socket) {
  const accept = createHash("sha1").update(req.headers["sec-websocket-key"] + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").digest("base64");
  socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
  let buf = Buffer.alloc(0);
  socket.on("data", (d) => {
    buf = Buffer.concat([buf, d]);
    if (buf.length < 2) return;
    let len = buf[1] & 0x7f, at = 2;
    if (len === 126) [len, at] = [buf.readUInt16BE(2), 4];
    const mask = buf[1] & 0x80 ? buf.subarray(at, at + 4) : null;
    if (mask) at += 4;
    if (buf.length < at + len) return;
    const data = Buffer.from(buf.subarray(at, at + len));
    if (mask) for (let i = 0; i < data.length; i++) data[i] ^= mask[i % 4];
    record({ proto: "ws", headers: req.headers, body: data.toString("latin1") });
    socket.end(Buffer.from([0x88, 0]));
  });
}

const server = plain === "--plain" ? plainServer(handle) : createSecureServer({ cert: readFileSync(cert), key: readFileSync(key), allowHTTP1: true }, handle);
server.on("upgrade", upgrade);
server.listen(0, "127.0.0.1", () => console.log(server.address().port));
