#!/usr/bin/env node
// Check that FILE.sig is a valid updater signature of FILE under the public key
// installed copies trust (plugins.updater.pubkey in src-tauri/tauri.conf.json),
// the way the updater checks it: minisign, Ed25519 over BLAKE2b-512 of the file
// ("ED") or over the file itself (legacy "Ed"), plus the signature over the
// trusted comment. Exit 0 if every file verifies, 1 otherwise.
//
// Usage: node scripts/verify-update-sig.mjs [--pubkey BASE64] FILE...
import { createHash, createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
let pubkeyB64;
if (args[0] === "--pubkey") {
  pubkeyB64 = args[1];
  args.splice(0, 2);
} else {
  const conf = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "tauri.conf.json");
  pubkeyB64 = JSON.parse(readFileSync(conf, "utf8")).plugins.updater.pubkey;
}
if (args.length === 0) {
  console.error("usage: verify-update-sig.mjs [--pubkey BASE64] FILE...");
  process.exit(2);
}

// Tauri wraps minisign's text files in one more layer of base64.
const lines = (b64) => Buffer.from(b64.trim(), "base64").toString("utf8").split(/\r?\n/);

function publicKey(b64) {
  const raw = Buffer.from(lines(b64)[1], "base64");
  if (raw.length !== 42 || raw.toString("latin1", 0, 2) !== "Ed") throw new Error("not a minisign public key");
  const der = Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), raw.subarray(10)]);
  return { id: raw.subarray(2, 10), key: createPublicKey({ key: der, format: "der", type: "spki" }) };
}

function check(file, pub) {
  const [, sigLine, trusted, globalLine] = lines(readFileSync(`${file}.sig`, "utf8"));
  const sig = Buffer.from(sigLine, "base64");
  const alg = sig.toString("latin1", 0, 2);
  if (sig.length !== 74 || (alg !== "ED" && alg !== "Ed")) return "malformed signature";
  if (!sig.subarray(2, 10).equals(pub.id)) return "signed with a different key";
  const data = readFileSync(file);
  const message = alg === "ED" ? createHash("blake2b512").update(data).digest() : data;
  if (!verify(null, message, pub.key, sig.subarray(10))) return "signature does not match the file";
  if (!trusted?.startsWith("trusted comment: ")) return "missing trusted comment";
  const comment = Buffer.from(trusted.slice("trusted comment: ".length), "utf8");
  if (!verify(null, Buffer.concat([sig.subarray(10), comment]), pub.key, Buffer.from(globalLine ?? "", "base64"))) {
    return "trusted comment was changed";
  }
  return null;
}

const pub = publicKey(pubkeyB64);
let failed = false;
for (const file of args) {
  let problem;
  try {
    problem = check(file, pub);
  } catch (e) {
    problem = e.message;
  }
  console.log(`${problem ? "FAIL" : "ok  "} ${file}${problem ? `: ${problem}` : ""}`);
  failed ||= Boolean(problem);
}
process.exit(failed ? 1 : 0);
