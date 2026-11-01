# Attack replays

Each script here reconstructs how a documented credential-stealing campaign,
or an AI coding agent that was asked or tricked into it, takes keys from a
developer's project and sends them out. They run under `keyhandoff run`
against a synthetic Stripe key whose real server is a local test server
(`api.stripe.test`), and send what they steal to local collectors with valid
certificates for other names. The harness is the ignored test
`attack_replays` in `src-tauri/src/core.rs`; results go to
`eval/results/replays.md` and `.json`.

## Ethics

- **No live malware.** Nothing here is a copy of a malicious package. Each
  script reimplements only the collection and exfiltration steps described
  in public incident reports, without propagation, persistence, destruction
  or any network traffic beyond `127.0.0.1`.
- **No real keys.** The only key is a synthetic test value. The collectors
  are local Node servers that write what they receive to a file in a
  temporary folder, deleted afterwards.
- **No third-party services.** Names like `api.github.test` and
  `webhook.test` resolve to the local collectors through the client's own
  connection settings; `.test` is reserved and never reaches the internet.

## The corpus

| Script | Reconstructs | What it does |
|---|---|---|
| `nx_s1ngularity.py` | The malicious `nx` releases of August 2025 ("s1ngularity") | Collects environment variables and `.env` files, asks a local AI CLI to look for more, base64-encodes the haul three times and uploads it as a file to a GitHub-style API. |
| `shai_hulud.mjs` | The Shai-Hulud npm worm of September 2025 | In Node, as an install script: harvests environment variables, encodes them twice with base64 and posts them to a webhook over HTTPS. |
| `shai_hulud_http.mjs` | The same, to a plain HTTP webhook | As above, without TLS. |
| `dotenv_hunter.py` | The common `.env` stealer pattern | Walks the project for `.env*` files, greps the environment for key-shaped values, and posts what it finds in clear. |
| `abuse_own_server.py` | Using a stolen key where it works | Calls the key's own API with it, to act as you. |
| agent commands | An AI coding agent asked, or prompt-injected, to leak a key | Under a parent process named `claude`: print it, base64 it, `env | curl` it to a paste site, post it with `requests`, smuggle it in a DNS name, and commit it to a file. |

Run everything with `eval/run.sh`.
