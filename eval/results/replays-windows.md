# Attack replays

Reconstructed stealers and AI-agent leaks (see eval/replays/README.md) under `keyhandoff run`, with a synthetic Stripe key whose server is `api.stripe.test`. **Attacker held**: what the attacking code had in hand (what it saved, printed or committed). **Left the machine**: what the collector received, decoding up to three layers of base64. **Stopped**: the send was refused before a byte of it left, and KeyHandoff kept an alert.

On Windows only Node.js is hooked so far (PAR-08). The other replays get the real key, so what they hold and send shows the exposure there; the agent replays run as plain commands, since the test can't start a wrapper shell on Windows.

| Replay | Attacker held | Left the machine | Stopped | Alert | Traced to its run |
|---|---|---|---|---|---|
| Nx "s1ngularity": env, .env and AI CLI inventory, triple base64, GitHub-style upload | the key | the key | no | - | - |
| Shai-Hulud: env harvest, double base64, HTTPS webhook (Node) | a placeholder | nothing | yes | node.exe shai_hulud.mjs → webhook.test | yes |
| Shai-Hulud: the same to a plain HTTP webhook (Node) | nothing usable (masked) | nothing | yes | node.exe shai_hulud_http.mjs → 127.0.0.1:54526 | - |
| .env hunter: grep files and environment, post in clear | the key | the key | no | - | - |
| Agent: print the key | nothing usable (masked) | - | no | - | - |
| Agent: base64 the key and print it | nothing usable (masked) | - | no | - | - |
| Agent: env | curl to a paste site | nothing usable (masked) | nothing | no | - | - |
| Agent: post the key to a webhook | nothing usable (masked) | the key | no | - | - |
| Agent: key in a DNS name | nothing usable (masked) | - | no | - | - |
| Agent: write the key into a committed file | the key | - | no | - | - |
| Abuse through the key's own server | nothing usable (masked) | the key | no | - | - |

The last row is a stated limitation, not a leak: code in the program can use the key at its own server, as any program allowed the key could. It never gets the key itself.

Afterwards, a teammate on the same team folder:

| Destination | Teammate's program |
|---|---|
| 127.0.0.1:54526 | refused |
| webhook.test | refused |

Regenerate: `cd src-tauri && cargo test --lib attack_replays -- --ignored --nocapture`.
