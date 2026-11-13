# ADR 0003: Keys never leave KeyHandoff

**Status:** proposed. Review before anyone starts.
**Date:** 2026-11-13

## Context

KeyHandoff today stops plain-text `.env` files and the generic attacks that scan for them. Against code running as the user it has three gaps, all of which came up on the owner's first day:

1. **Remember on this computer** keeps the vault key with DPAPI, so any program running as the user can decrypt the vault without asking. The 2am report needs Remember, because the vault is locked overnight.
2. **Most programs get the real key.** Stand-ins work only where we hook the TLS library: OpenSSL on Linux, and Node.js. Go, Java and Rust with rustls bring their own TLS, so there is nothing to hook. On Windows, Python gets the real key. Anything that gets the real key can write it out, which is how `recovered-key.txt` was made.
3. **Unattended jobs** need a person to click (FEAT-16).

A lab-grade bar ("a stolen laptop, malware running as the user, or a prompt-injected agent gets nothing usable") needs all three closed. It also has to stay simple: one install, and a fingerprint or PIN where a click is today.

## Decision

Three changes, which together close all three gaps.

### 1. The vault moves into a KeyHandoff service

A small background service, running under its own Windows service account, holds the vault. The window you see becomes only its screen. The service's files are readable only by that account, so a program running as you can't open them, with or without Remember. The vault key is sealed to this computer's TPM (which this PC has, version 2.0, already on). So the service can open the vault at startup without a passphrase, and a copied disk opens nowhere else.

On Linux the service is a systemd system service under its own user. On macOS it's a launchd daemon with its key in the system keychain. Secure Enclave needs a T2 chip or Apple silicon, so a 2017 MacBook Air uses the keychain alone.

Installing the service takes one administrator prompt, once.

**Remember on this computer goes away.** It isn't needed: the service is always open, and no user-level program can read its files.

### 2. Stand-ins for every language, through a local proxy

Instead of hooking each TLS library, `keyhandoff run` (and the loaders) points the program at a proxy inside the service:

- The program gets **stand-ins** in its environment, plus `HTTPS_PROXY` set to the service. Most HTTP clients honour it: reqwest, Go's net/http, Java (through `JAVA_TOOL_OPTIONS`), requests, httpx, curl and Node.js.
- The program's encrypted connection ends at the proxy. The proxy shows a certificate from a **local certificate authority whose name constraints allow only the hosts of your kept keys**, like `api.dialpad.com`. Even if someone stole that authority's key, it couldn't impersonate any other site. Its private key never leaves the service. The runner makes the program trust it through `SSL_CERT_FILE`, `REQUESTS_CA_BUNDLE`, `NODE_EXTRA_CA_CERTS` and `JAVA_TOOL_OPTIONS`, and through the user's certificate store for clients that read only the system store (Go and rustls-native-certs on Windows and macOS).
- The proxy checks that the connection comes from a process in that run, and that the stand-in belongs to that run. It replaces the stand-in with the real key, opens its own encrypted connection to the real server, checks that server's certificate, and forwards the request.
- A stand-in headed to any other host is a **tripwire**, as today.
- A program that ignores the proxy, or bundles its own fixed root certificates (rustls with webpki-roots), simply fails: the server receives a stand-in. Nothing leaks. The existing OpenSSL and Node hooks stay as a fallback for those cases.

The real key exists in exactly two places: inside the service, and inside the encrypted connection to its own verified server. No program ever holds it, so no program, package or agent can write it to a file. This replaces ADR 0002's Python hook (FEAT-14).

### 3. Approvals are confirmed with Windows Hello

"Always allow" and unattended-job grants are confirmed with Windows Hello: fingerprint, face or PIN, on the system's own secure prompt. A program running as you can click buttons in the KeyHandoff window, but it can't answer Windows Hello. Without Hello set up, it falls back to the passphrase. "Just this once" stays a single click.

Unattended jobs (FEAT-16) become a grant on a key row: this script, any arguments, these keys. You confirm it once with Hello, and the service serves it at 2am with nobody there.

## What this does and doesn't stop

| Attacker | Today | After |
|---|---|---|
| Something scanning for `.env` files | Gets pointers | Gets pointers |
| A malicious package inside an approved program | Gets the real key (Windows Python), or a stand-in | Gets a stand-in |
| A prompt-injected agent | Stand-in on Linux and Node, real key elsewhere | Stand-in everywhere |
| Malware running as you, Remember on | Decrypts the vault | Can't read the service's files; can't answer Hello |
| Malware running as you, using an approved program | Uses the key via the real program | Same: it can *use* the key on its own server, but can't take it. Tripwires and the audit log record it. |
| A stolen laptop | Vault needs the passphrase (or DPAPI with Remember) | Vault is sealed to the TPM and the service account |
| An administrator or the kernel | Wins | Still wins (out of scope) |

## Costs

- One administrator prompt at install. On a managed work PC, IT may have to allow it.
- The proxy has to speak HTTP/1.1 and HTTP/2 (hyper does both) and handle WebSockets.
- Adding the local authority to the user certificate store shows a Windows confirmation dialog once.
- Plain-text export becomes impossible, by design. Backup is an encrypted vault copy plus the recovery code.

## Not in this decision

Org features (a central policy file, sourcing keys from HashiCorp Vault / 1Password / AWS, revocation) are a separate plan. One piece is cheap and comes here: every grant, run, swap and tripwire goes to the system log (Windows Event Log, journald, macOS unified log), so a company's existing security tools pick it up with no new server.
