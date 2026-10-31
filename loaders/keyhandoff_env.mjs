// Fill environment variables from KeyHandoff pointers in .env.
//
// A .env line such as `DIALPAD_API_KEY=handoff:Dialpad` names a key kept in
// KeyHandoff instead of holding it. `load()` asks the running app for every
// pointed-to key at once. The first time a program asks from a folder,
// KeyHandoff asks you to allow it. The values are set in `process.env` for
// this process only; nothing is written to disk.
//
//     import { load } from "./keyhandoff_env.mjs"; await load();
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
export async function load(path = ".env", profile = undefined) {
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
  return vars;
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
