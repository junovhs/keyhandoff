# KeyHandoff

**API keys, hands off.**

KeyHandoff is a small desktop app for handing someone an API key, or any other secret, without pasting it into a chat or leaving it in a plain file. There are no servers and no accounts. Requests and keys travel as ordinary files over whatever channel you already use.

> **Status: pre-release.** It has not had a security audit. Use it with test keys for now.

## How it works

1. **Ask.** You click **Request a key** and send the request file to whoever has the key.
2. **Catch.** They open the request and copy the key from the provider's page. KeyHandoff catches that one copy and wipes the clipboard.
3. **Seal.** They name the key, and it's encrypted so only your computer can open it, and only once. They send you the file and keep nothing.
4. **Keep.** You open the file. The key goes into your vault, and every copy of the sealed file becomes permanently unreadable.

You can also catch a key for yourself with **Add your own**. KeyHandoff has no way to show a key on screen.

**Use a key in a program.** Your `.env` holds a pointer instead of the key, such as `DIALPAD_API_KEY=handoff:Dialpad`. A one-line loader, copied next to your script, asks the running app for it: [`keyhandoff_env.py`](loaders/keyhandoff_env.py) for Python (`keyhandoff_env.load()`) or [`keyhandoff_env.mjs`](loaders/keyhandoff_env.mjs) for Node.js (`await load()`). The first time a program asks from a folder, KeyHandoff shows which program and folder, and you choose Always allow, Just this once or Deny. The value goes into that process's environment and is never written to disk.

Typing a key into a field is still being built.

## What it protects, and what it doesn't

The full list, with the test or design rule behind each claim, is in [docs/threats.md](docs/threats.md).

**It removes the places a key normally lingers:**

- Chat and email only ever carry a sealed file, which is useless to anyone else.
- The sender keeps nothing, and a sealed file can't be opened twice.
- Keys are never displayed, and the app's screens never receive them; only the Rust core handles the values.
- The app has no servers, accounts, telemetry or network code.

**What it can't do:**

- **The copy moment.** A key copied from a web page is on the system clipboard, unencrypted, for the moment before KeyHandoff wipes it. Another program watching the clipboard could read it in that moment; on Linux (X11), one that watches will, every time ([measured](docs/capture-tests.md)). If you suspect your computer is compromised, replace the key.
- **Malware running as you.** On Windows the vault is protected with DPAPI, and on Linux with the Secret Service, or a private file if there is none. Other programs running under your account can ask those to unlock it. Hardware-backed protection is planned.
- **A program you allowed.** Once a program has a key, it can do anything with it, including print it or save it. The prompt names the program and folder so you can decide, but anything running as you could start that same program in that folder.
- **A compromised computer.** No app can fully protect a secret on a machine an attacker controls.
- **Lost computers.** Keys are tied to one computer and there is no recovery. Replace the keys with the provider.

## Platforms

Windows and Linux (X11). macOS and Wayland aren't supported.

## Build

You need [Rust](https://rustup.rs) and [Node.js](https://nodejs.org), plus the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your system.

```
npm install
npm run tauri dev                   # run with hot reload
npm run tauri build                 # installer in src-tauri/target/release/bundle
npm run tauri build -- --no-bundle  # just the binary
```

To try two identities on one computer, run each with its own profile: `keyhandoff --profile second`, or set `KEYHANDOFF_PROFILE=second`.

## Tests

```
cd crates/seal && cargo test
cd src-tauri && cargo test
cd src-tauri && cargo test -- --ignored --test-threads=1   # real clipboard checks
```

Tests only ever use synthetic keys. On Linux the real clipboard checks need `DISPLAY` and `xclip`.

## Documentation

- [How it works](docs/how-it-works.md)
- [Architecture](docs/architecture.md)
- [Sealing format and key storage](docs/sealing-format.md)
- [Design rules](docs/rules/keyhandoff.md)

## License

KeyHandoff is free and open to read, including for use at work.

- The app is under the [PolyForm Shield License 1.0.0](LICENSE.md), which doesn't allow using it in a product that competes with KeyHandoff. It isn't an OSI-approved open source license.
- The sealing library, [`crates/seal`](crates/seal), is under the [Apache License 2.0](crates/seal/LICENSE), so it can be reviewed and reused freely.

Copyright © 2026 Spencer Nunamaker. Outside contributions aren't being accepted right now.

To report a security problem, see [SECURITY.md](SECURITY.md).
