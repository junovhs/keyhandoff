// KeyHandoff's hook for Node.js, loaded by `keyhandoff run` with
// `NODE_OPTIONS=--require`. Node links its own OpenSSL, out of reach of the
// TLS hook keyhandoff run preloads (hook/keyhandoff_tls.c), so this does the
// same job one level up: it wraps the write path of every TLS socket, and
// when a write holds a placeholder (as written, or encoded like the C hook
// knows) it waits for the handshake, checks the server's certificate chain
// itself (Node's root store plus the key's own CA, never the socket's
// `authorized`, which the program's own options decide) and its name
// against the key's hosts. Verified: it asks KeyHandoff for the key, swaps
// it in and writes. Anywhere else: the write is refused before a byte
// leaves, and KeyHandoff is told where it was going. HTTP/2 (Node's http2
// module) compresses headers inside Node and isn't covered.
//
// The C hook, loaded in this process too, leaves KEYHANDOFF_HOOK in the
// environment for this file; it's read once and removed here.
"use strict";

const ENV = "KEYHANDOFF_HOOK";
const value = process.env[ENV];
if (value && /^[0-9a-f]{64}:/.test(value)) {
  delete process.env[ENV];
  install(value.slice(0, 64), value.slice(65));
  // On Windows there's no C hook to hand the token to programs this one
  // starts, so do it here, as the C hook does on Linux: a Node program
  // under `npm run` or started by another Node program gets this hook too.
  if (process.platform === "win32") passOn(value);
}

/** Put KEYHANDOFF_HOOK back into the environment of every process this one starts. */
function passOn(value) {
  const { ChildProcess } = require("node:child_process");
  const spawn = ChildProcess.prototype.spawn;
  ChildProcess.prototype.spawn = function (options) {
    if (options && Array.isArray(options.envPairs) && !options.envPairs.some((p) => p.startsWith(ENV + "="))) {
      options.envPairs = [...options.envPairs, `${ENV}=${value}`];
    }
    return spawn.call(this, options);
  };
}

function install(token, socketPath) {
  const crypto = require("node:crypto");
  const fs = require("node:fs");
  const net = require("node:net");
  const tls = require("node:tls");

  /** One request to KeyHandoff; resolves to its reply, every line. */
  function ask(line) {
    return new Promise((resolve) => {
      const chunks = [];
      const conn = net.connect(socketPath, () => conn.end(line));
      conn.on("data", (d) => chunks.push(d));
      conn.on("close", () => resolve(Buffer.concat(chunks).toString("latin1")));
      conn.on("error", () => resolve(""));
    });
  }

  let config;
  /** The run's placeholders and the destinations to refuse, asked for once. */
  function load() {
    config ??= ask(`{"hook":"${token}"}\n`).then((reply) => {
      const swaps = [];
      const blocked = [];
      for (const line of reply.split("\n")) {
        if (line.startsWith("!")) blocked.push(line.slice(1).toLowerCase());
        else if (line.includes("\t")) {
          const [placeholder, hosts, ca] = line.split("\t");
          swaps.push({ placeholder, hosts: hosts.split(",").filter(Boolean), ca: ca || null, forms: forms(placeholder) });
        }
      }
      return { swaps, blocked };
    });
    return config;
  }

  // Encoded forms, as the C hook knows them: base64 three levels deep at
  // each offset (the first three, one level, can be swapped), URL-safe
  // base64, hex, percent-encoding and reversed.
  function base64Forms(text, alphabet, depth, out) {
    for (let offset = 0; offset < 3; offset++) {
      const encoded = Buffer.concat([Buffer.alloc(offset), Buffer.from(text, "latin1")]).toString(alphabet).replace(/=+$/, "");
      const skip = offset ? Math.ceil((offset * 8) / 6) : 0;
      const keep = Math.floor(((text.length + offset) * 8) / 6);
      if (keep - skip < 12) continue;
      const form = encoded.slice(skip, keep);
      out.push(form);
      if (depth > 1) base64Forms(form, alphabet, depth - 1, out);
    }
    return out;
  }
  function forms(p) {
    const out = base64Forms(p, "base64", 1, []);
    const hex = Buffer.from(p, "latin1").toString("hex");
    out.push(hex, hex.toUpperCase(), hex.replace(/../g, "%$&"), hex.toUpperCase().replace(/../g, "%$&"), [...p].reverse().join(""));
    for (const level1 of out.slice(0, 3)) base64Forms(level1, "base64", 2, out);
    base64Forms(p, "base64url", 1, out);
    return out;
  }
  /** 1: as written; 2: one level of base64; 3: another form; 0: none. */
  function found(swap, text) {
    if (text.includes(swap.placeholder)) return 1;
    const i = swap.forms.findIndex((f) => text.includes(f));
    return i < 0 ? 0 : i < 3 ? 2 : 3;
  }

  // Trusted roots: Node's, and a key's own CA file.
  const roots = new Map();
  function rootsFor(ca) {
    const k = ca ?? "";
    if (!roots.has(k)) {
      const pems = [...tls.rootCertificates];
      if (ca) {
        try {
          pems.push(...(fs.readFileSync(ca, "latin1").match(/-----BEGIN CERTIFICATE-----[\s\S]+?-----END CERTIFICATE-----/g) ?? []));
        } catch {}
      }
      roots.set(k, pems.map((p) => new crypto.X509Certificate(p)));
    }
    return roots.get(k);
  }

  /** The key host the server's verified certificate is valid for, or null. */
  function verifiedHost(socket, swap) {
    let cert = socket.getPeerCertificate(true);
    if (!cert?.raw) return null;
    const chain = [];
    const seen = new Set();
    while (cert?.raw && !seen.has(cert.fingerprint256)) {
      seen.add(cert.fingerprint256);
      chain.push(new crypto.X509Certificate(cert.raw));
      cert = cert.issuerCertificate;
    }
    const trusted = rootsFor(swap.ca);
    const now = Date.now();
    let ok = false;
    for (let i = 0; i < chain.length && !ok; i++) {
      const c = chain[i];
      if (now < Date.parse(c.validFrom) || now > Date.parse(c.validTo)) return null;
      if (trusted.some((t) => t.fingerprint256 === c.fingerprint256) || trusted.some((t) => c.checkIssued(t) && c.verify(t.publicKey))) {
        ok = true;
        break;
      }
      const next = chain[i + 1];
      if (!next || !next.ca || !c.checkIssued(next) || !c.verify(next.publicKey)) return null;
    }
    return ok ? (swap.hosts.find((h) => chain[0].checkHost(h)) ?? null) : null;
  }

  async function keyFor(swap, host) {
    const reply = await ask(`{"swap":{"token":"${token}","placeholder":"${swap.placeholder}","host":"${host}"}}\n`);
    const m = /^ok ([0-9a-f]+)\n/.exec(reply);
    return m && m[1].length === swap.placeholder.length * 2 ? Buffer.from(m[1], "hex").toString("latin1") : null;
  }

  function say(what, destination) {
    try {
      fs.writeSync(2, `KeyHandoff ${what} ${destination}.\n`);
    } catch {}
  }

  function report(swap, destination) {
    const safe = destination.replace(/[^A-Za-z0-9.\-:\[\]_]/g, "");
    say("stopped a key's stand-in going to", destination);
    return ask(`{"alert":{"token":"${token}","placeholder":"${swap.placeholder}","destination":"${safe}"}}\n`);
  }

  /** Every standard base64 run that decodes to text holding the placeholder, with the key swapped in, the same length. */
  function swapBase64(text, swap, key) {
    return text.replace(/[A-Za-z0-9+/]{16,}={0,2}/g, (run) => {
      const plain = Buffer.from(run, "base64").toString("latin1");
      if (!plain.includes(swap.placeholder)) return run;
      const again = Buffer.from(plain.split(swap.placeholder).join(key), "latin1").toString("base64");
      const out = run.endsWith("=") ? again : again.replace(/=+$/, "");
      return out.length === run.length ? out : run;
    });
  }

  const secure = (socket) =>
    socket._secureEstablished
      ? Promise.resolve()
      : new Promise((resolve, reject) => {
          socket.once("secureConnect", resolve);
          socket.once("secure", resolve);
          socket.once("error", reject);
          socket.once("close", () => reject(new Error("closed")));
        });

  const original = net.Socket.prototype._writeGeneric;
  tls.TLSSocket.prototype._writeGeneric = function (writev, data, encoding, cb) {
    const socket = this;
    const pieces = writev
      ? data.map((c) => (typeof c.chunk === "string" ? Buffer.from(c.chunk, c.encoding) : Buffer.from(c.chunk)))
      : [typeof data === "string" ? Buffer.from(data, encoding) : Buffer.from(data)];
    const refuse = (message) => {
      const error = Object.assign(new Error(message), { code: "ECONNREFUSED" });
      cb(error);
      socket.destroy(error);
    };
    load().then(async ({ swaps, blocked }) => {
      const destination = (socket.servername || socket._host || socket.remoteAddress || "an unknown server").toLowerCase();
      if (blocked.includes(destination)) {
        say("refused a connection to", destination);
        return refuse("KeyHandoff refused this destination");
      }
      const texts = pieces.map((p) => p.toString("latin1"));
      const hits = swaps.filter((s) => texts.some((t) => found(s, t)));
      if (!hits.length) return original.call(socket, writev, data, encoding, cb);
      try {
        await secure(socket);
      } catch {
        return original.call(socket, writev, data, encoding, cb);
      }
      // The name the connection was made for is only known once it's up.
      const named = (socket.servername || destination).toLowerCase();
      for (const swap of hits) {
        const host = verifiedHost(socket, swap);
        if (!host) {
          await report(swap, named);
          return refuse("KeyHandoff stopped a key's stand-in going somewhere else");
        }
        const key = await keyFor(swap, host);
        if (!key) continue; // the right server, but the run is over: the placeholder goes
        for (let i = 0; i < texts.length; i++) texts[i] = swapBase64(texts[i].split(swap.placeholder).join(key), swap, key);
      }
      const out = texts.map((t) => Buffer.from(t, "latin1"));
      if (writev) original.call(socket, true, out.map((chunk) => ({ chunk, encoding: "buffer" })), "", cb);
      else original.call(socket, false, out[0], "buffer", cb);
    }, () => original.call(socket, writev, data, encoding, cb));
  };
}
