// Fill environment variables from KeyHandoff pointers in .env.
//
// A .env line such as `DIALPAD_API_KEY=handoff:Dialpad` names a key kept in
// KeyHandoff instead of holding it. `load()` asks the running app for every
// pointed-to key at once. The first time a program asks from a folder,
// KeyHandoff asks you to allow it; if it is locked, it comes forward and this
// waits for you to unlock it. The values are set in `process.env` for
// this process only; nothing is written to disk.
//
//     import { load } from "./keyhandoff_env.mjs"; await load();
//
// What this process prints through process.stdout and process.stderr then
// passes through a filter that shows a key, plain, base64, hex, URL-encoded or
// reversed, as `[KeyHandoff: Name hidden]`. It catches a key printed by
// accident. Child processes that write to the terminal directly aren't
// covered; `keyhandoff run` covers them. Pass `{ mask: false }` to leave
// output alone.
//
// No dependencies: copy this file next to your script. JavaScript strings
// can't be wiped, so the value stays in this process's memory until it exits.

import { readFileSync } from "node:fs";
import { connect } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";

const PREFIX = "handoff:";
const NOT_RUNNING = "KeyHandoff isn't running. Open it and try again.";

export class KeyHandoffError extends Error {}

/**
 * Set every variable whose value is a `handoff:` pointer, from the
 * environment or from the .env file at `path`. A variable already set to a
 * real value is left alone. Resolves to the names of the variables set.
 */
export async function load(path = ".env", profile = undefined, { mask = true } = {}) {
  const pointers = {};
  for (const [name, value] of Object.entries(process.env)) {
    if (value.startsWith(PREFIX)) pointers[name] = value.slice(PREFIX.length).trim();
  }
  for (const [name, value] of readEnv(path)) {
    if (value.startsWith(PREFIX) && (!(name in process.env) || name in pointers)) {
      pointers[name] = value.slice(PREFIX.length).trim();
    }
  }
  const vars = Object.keys(pointers).sort();
  if (vars.length === 0) return [];
  const names = [...new Set(Object.values(pointers))].sort();
  const reply = await ask(names, profile || process.env.KEYHANDOFF_PROFILE || "default");
  if ("error" in reply) throw new KeyHandoffError(reply.error);
  for (const name of vars) process.env[name] = reply.values[pointers[name]];
  if (mask) maskOutput(reply.values);
  return vars;
}

// Hiding printed keys. Same rules as KeyHandoff's own masker (src-tauri/src/mask.rs).

const FRAGMENT = 12;

function forms(key) {
  const k = Buffer.from(key);
  const out = [k, Buffer.from(k).reverse(), Buffer.from(k.toString("hex")), Buffer.from(k.toString("hex").toUpperCase())];
  for (const encoding of ["base64", "base64url"]) {
    for (let offset = 0; offset < 3; offset++) {
      const encoded = Buffer.from(Buffer.concat([Buffer.alloc(offset), k]).toString(encoding));
      const skip = offset ? Math.ceil((offset * 8) / 6) : 0;
      const keep = Math.floor(((offset + k.length) * 8) / 6);
      if (keep > skip) out.push(encoded.subarray(skip, keep));
    }
  }
  const pct = (upper) => Buffer.from([...k].map((b) => "%" + b.toString(16).padStart(2, "0")[upper ? "toUpperCase" : "toLowerCase"]()).join(""));
  out.push(pct(true), pct(false));
  return out.filter((f) => f.length > 0);
}

class Masker {
  constructor(keys) {
    this.forms = Object.entries(keys).flatMap(([name, value]) => forms(value).map((form) => [name, form]));
    this.starts = new Set();
    for (const [, f] of this.forms) {
      const w = Math.min(FRAGMENT, f.length);
      for (let i = 0; i + w <= f.length; i++) this.starts.add(f.subarray(i, i + w).toString("latin1"));
    }
    this.widths = [...new Set(this.forms.map(([, f]) => Math.min(FRAGMENT, f.length)))].sort((a, b) => b - a);
    this.held = Buffer.alloc(0);
  }

  within(bytes) {
    const hit = this.forms.find(([, f]) => f.includes(bytes));
    return hit ? hit[0] : null;
  }

  push(data, last = false) {
    const buf = Buffer.concat([this.held, Buffer.from(data)]);
    const out = [];
    let i = 0;
    let from = 0;
    while (i < buf.length) {
      const w = this.widths.find((w) => i + w <= buf.length && this.starts.has(buf.subarray(i, i + w).toString("latin1")));
      if (w !== undefined) {
        let end = i + w;
        while (end < buf.length && this.within(buf.subarray(i, end + 1))) end++;
        if (end === buf.length && !last) break;
        out.push(buf.subarray(from, i), Buffer.from(`[KeyHandoff: ${this.within(buf.subarray(i, end)) ?? "A key"} hidden]`));
        i = from = end;
        continue;
      }
      if (!last && buf.length - i < FRAGMENT && this.within(buf.subarray(i))) break;
      i++;
    }
    out.push(buf.subarray(from, i));
    this.held = Buffer.from(buf.subarray(i));
    return Buffer.concat(out);
  }
}

function maskOutput(values) {
  for (const stream of [process.stdout, process.stderr]) {
    const masker = new Masker(values);
    const write = stream.write.bind(stream);
    stream.write = (chunk, encoding, callback) => {
      if (typeof encoding === "function") [callback, encoding] = [encoding, undefined];
      const bytes = typeof chunk === "string" ? Buffer.from(chunk, encoding) : chunk;
      return write(masker.push(bytes), callback);
    };
    process.once("exit", () => write(masker.push(Buffer.alloc(0), true)));
  }
}

function* readEnv(path) {
  let text;
  try {
    text = readFileSync(path, "utf8");
  } catch (e) {
    if (e.code === "ENOENT") return;
    throw e;
  }
  for (let line of text.split(/\r?\n/)) {
    line = line.trim();
    if (line.startsWith("export ")) line = line.slice("export ".length).trimStart();
    const eq = line.indexOf("=");
    if (!line || line.startsWith("#") || eq < 0) continue;
    const name = line.slice(0, eq).trim();
    let value = line.slice(eq + 1).trim();
    if (value.length >= 2 && value[0] === value.at(-1) && `'"`.includes(value[0])) {
      value = value.slice(1, -1);
    } else if (value.includes(" #")) {
      value = value.slice(0, value.indexOf(" #")).trimEnd();
    }
    yield [name, value];
  }
}

function where(profile) {
  if (process.platform === "win32") {
    const user = (process.env.USERNAME ?? "").replace(/[^A-Za-z0-9]/g, "");
    return `\\\\.\\pipe\\handoff-${user}-${profile}-keys`;
  }
  return join(process.env.XDG_RUNTIME_DIR || tmpdir(), `handoff-${profile}-keys.sock`);
}

function ask(names, profile) {
  return new Promise((resolve, reject) => {
    const conn = connect(where(profile));
    let data = "";
    let answered = false;
    conn.setEncoding("utf8");
    conn.on("connect", () => conn.write(JSON.stringify({ names }) + "\n"));
    conn.on("data", (chunk) => {
      data += chunk;
      const end = data.indexOf("\n");
      if (end < 0) return;
      answered = true;
      conn.end();
      try {
        resolve(JSON.parse(data.slice(0, end)));
      } catch {
        reject(new KeyHandoffError("KeyHandoff sent a reply that couldn't be read."));
      }
    });
    conn.on("error", (e) => {
      if (!answered) reject(new KeyHandoffError(e.code === "ENOENT" || e.code === "ECONNREFUSED" ? NOT_RUNNING : e.message));
    });
    conn.on("close", () => {
      if (!answered) reject(new KeyHandoffError("KeyHandoff closed the connection."));
    });
  });
}
