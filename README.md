# KeyHandoff

**API keys, hands off.**

KeyHandoff is a small desktop app for handing someone an API key, or any other secret, without pasting it into a chat or leaving it in a plain file. There are no servers and no accounts. Requests and keys travel as ordinary files over whatever channel you already use.

Encrypting a key at rest is the easy part. The real exposure comes from people handling it: looking at it, copying it, pasting it into a chat, saving it in a `.env` file. KeyHandoff keeps custody of the key from the provider's page onward, so people and their tools handle it as little as possible.

> **Status: pre-release.** It has not had a security audit. Use it with test keys for now.

## What you handle

| Thing | What it is | Secret inside? |
|---|---|---|
| **Request file** (`Key request from Ann.handoff`) | "Ann is asking for a key." Holds Ann's public identity and a fresh one-time lock. | No |
| **Sealed key file** (`Key for Ann.handoff`) | The key, locked so only Ann's computer can open it, and only once. | Yes, encrypted |
| **Kept key** | An entry in your vault: its name and source, never its value. | Stored encrypted |

Both files hold damage-resistant text, so they can also be pasted as text when a channel can't carry files.

## Getting a key from someone

1. **Ask.** Ann clicks **Request a key**. A request file goes on the clipboard, ready to paste into a chat.
2. **Catch.** Ben opens the file. KeyHandoff shows who is asking and their computer's fingerprint, and Safe Copy turns on. Ben copies the key on the provider's page, and KeyHandoff catches that one copy and wipes the clipboard.
3. **Seal.** Ben names the key. It's encrypted so only Ann's computer can open it, and only once. Ben sends back the sealed file and keeps nothing.
4. **Keep.** Ann opens the file. The key goes into her vault, and every copy of the sealed file, in the chat or anywhere else, can never be opened again.

To keep a key of your own, click **Add your own**, copy the key on the provider's page and name it. No files are involved. KeyHandoff has no way to show a key on screen.

## Safe Copy

Safe Copy reads the clipboard only after you ask it to. It catches one copy, then switches off, or stops after 60 seconds. Nothing watches the clipboard the rest of the time. After a catch it wipes the clipboard, and removes the copy from clipboard history where the system allows it.

When it turns on, it checks the running programs against a short list of known clipboard managers, such as Ditto or CopyQ, and names any it finds, since they can read the key as you copy it. Clipboard extensions built into a desktop shell don't show up as programs, so an empty list isn't a promise.

## Keep times

When you accept or add a key you keep it **Once**, for **30 days**, or until you remove it. Your last choice becomes the default. A key kept Once is destroyed after a program first uses it.

## Using a key in a program

1. **Point at the key.** Your `.env` holds `DIALPAD_API_KEY=handoff:Dialpad` instead of the key. **Copy .env line** on the key's row gives you that line, with a variable name made from the key's name; rename the variable if your code expects another. The name after `handoff:` matches a kept key, ignoring case. Names are unique, so a pointer always means one key.
2. **Load it.** Copy a one-line loader next to your script: [`keyhandoff_env.py`](loaders/keyhandoff_env.py) for Python (`keyhandoff_env.load()`) or [`keyhandoff_env.mjs`](loaders/keyhandoff_env.mjs) for Node.js (`await load()`). Call it once at startup, after any `.env` loader you already use, such as `load_dotenv()` or `dotenv`. Your code then reads `os.environ` or `process.env` as before. It asks the running app for every pointed-to key at once, over a local socket (a named pipe on Windows). The loader runs inside your program and sets the variable there, rather than launching the program with the key, because a program's starting environment can be read by other programs running as you.
3. **Allow it.** KeyHandoff asks the operating system which program is asking and which folder it runs in. The first time, the window shows both, and you choose **Always allow**, **Just this once** or **Deny**. Always allow covers that program, in that folder, for those keys. The same program in another folder asks again, because allowing `python` everywhere would allow every script.
4. **Use it.** The value goes into the program's own environment. Nothing is written to disk.

Typing a key into a field, without the clipboard, is still being built.

## What it protects, and what it doesn't

Chat and email only ever carry a sealed file. The sender keeps nothing, a sealed file opens once, keys are encrypted on disk and never shown, and there are no servers, accounts, telemetry or network code.

It can't stop a program that's watching the clipboard from reading a key at the moment you copy it, or stop anything running as you from reaching the vault. A program you allow can do anything with the key it gets. **[docs/threats.md](docs/threats.md)** has the full list, with the test or design rule behind each claim.

The sender can't control what the receiver does with a key afterwards, and KeyHandoff doesn't pretend otherwise. It isn't trying to be a general password manager, a phone app or a service with accounts.

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

- [What it protects, and what it doesn't](docs/threats.md)
- [Capture tests: the copy moment](docs/capture-tests.md)
- [Architecture](docs/architecture.md)
- [Sealing format and key storage](docs/sealing-format.md)
- [Design rules](docs/rules/keyhandoff.md)

## License

KeyHandoff is free and open to read, including for use at work.

- The app is under the [PolyForm Shield License 1.0.0](LICENSE.md), which doesn't allow using it in a product that competes with KeyHandoff. It isn't an OSI-approved open source license.
- The sealing library, [`crates/seal`](crates/seal), is under the [Apache License 2.0](crates/seal/LICENSE), so it can be reviewed and reused freely.

Copyright © 2026 Spencer Nunamaker. Outside contributions aren't being accepted right now.

To report a security problem, see [SECURITY.md](SECURITY.md).
