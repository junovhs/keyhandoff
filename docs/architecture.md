# Architecture

## Layout

```
crates/seal/       Standalone Rust crate: identity, requests, seal and open, one-time keys,
                   the .handoff format and armored text. No Tauri, UI or OS keychain.
src/               React + TypeScript screens. Never receive a secret value.
src-tauri/src/
  lib.rs           Window and Tauri commands (names, states and ids only)
  core.rs          Every flow: ask, Safe Copy, seal, accept, add, give keys to
                   allowed programs. Holds the secrets.
  pointer.rs       What programs asking for keys send and get back
  store.rs         Profile files sealed under the master key; the locked profile
  slots.rs         keyslots.bin: the master key wrapped by passphrase and recovery code
  session.rs       Locked or open: unlock, lock on sleep, screen lock or idle
  vault.rs         Kept keys, keep times, expiry
  os/              Per-OS backends: clipboard capture, offer and read, master key,
                   drag-out, one instance per profile, the key socket or pipe
                   and who is asking (linux/, windows/)
loaders/           One-line loaders programs use to fill handoff: pointers
scripts/           Development helpers
```

## Stack

Tauri, with React and TypeScript screens and a Rust backend.

**Secret values live only in Rust:** keys, one-time keys and caught clipboard text. Tauri commands give the screens names, states and ids. A screen asks to "seal what you caught for this request", and it never passes or receives the secret.

## Sealing model

See [sealing-format.md](sealing-format.md) for the details.

- **Identity:** each computer generates a long-term signing key pair on first launch.
- **Request:** the requester's name, long-term public key and a fresh one-time public key, signed by the long-term key. The one-time private half stays on the requester's computer.
- **Seal:** the sender encrypts the secret to the one-time public key with a fresh ephemeral key, and signs the result.
- **Open:** the receiver decrypts with the one-time private key, stores the secret in the vault, then destroys the one-time key.
- **Expiry:** unopened requests stop working after 7 days by default, by destroying their one-time key.

## Key storage

Each profile has a 32-byte master key. `identity.bin`, `pending.bin`, `vault.bin`, `approvals.bin`, `contacts.bin` and `teams.bin` are sealed under it with ChaCha20-Poly1305, with the file name as associated data. Inside `vault.bin`, each kept secret is sealed again under the vault key with its item id as associated data, so a loaded vault holds ciphertext until a key is used.

The master key is stored only in `keyslots.bin`, wrapped once per way in, and any one slot opens it:

- **Passphrase:** Argon2id (256 MiB, 3 passes, 4 lanes) turns the passphrase into a key that seals the master key with ChaCha20-Poly1305.
- **Recovery code:** 160 random bits shown once at setup, as eight groups of four Crockford base32 characters. It opens the vault and sets a new passphrase.

The app starts locked (`session.rs`), and the core starts only once a slot opens. It locks again on sleep, screen lock, the lock button, or after `lock_after_minutes` without use if set; locking drops the core and the master key with it. A program asking for a key while it's locked brings the window forward and waits up to two minutes for the unlock. Files opened while locked open right after. Profiles from before key slots kept the master key in DPAPI (Windows) or the Secret Service or a `0600` file (Linux), which hand it to any program running as you; on the first start the passphrase is set, the key moves into its slots and the old copy is deleted.

**Remember on this computer** (opt-in, from Security in the home footer) keeps a copy of the master key with the OS again: DPAPI on Windows, the Secret Service or a `0600` file on Linux. The vault then opens at launch without asking and doesn't lock on sleep or screen lock; turning it off deletes the copy. The passphrase and recovery code keep working either way.

Hardware slots are planned and add a kind each: a TPM 2.0 key behind a PIN, Windows Hello, and a FIDO2 security key. See [the research report](../reports/Hardware%20key%20protection%20everywhere.md).

Profile data lives in a `handoff` folder under the system's data directory. Each profile has its own data, master key and running instance.

## Safe Copy capture

- Arms only on a user action: opening a request, or Add.
- On arming, names known clipboard managers found in the process list (`/proc/*/comm` on Linux, a Toolhelp snapshot on Windows).
- Takes the first text clipboard change, then disarms. Times out after 60 seconds.
- Never renders the value. It's zeroized when dropped.
- Text dropped onto the window isn't accepted, because the webview would receive it in TypeScript.

**Windows:** a message-only window with `AddClipboardFormatListener`. The first `CF_UNICODETEXT` change is read, our copy zeroed and the clipboard emptied. The matching Win+V entry is then looked for and deleted on a single-threaded WinRT thread. `ClearHistory` is the fallback only when the one entry can't be deleted, and that fallback is shown to the user. A copy made by another app can't be kept out of history up front.

**Linux (X11):** While listening, KeyHandoff owns the clipboard with harmless bait text. X11 keeps no copy, so a program reading the clipboard must ask KeyHandoff; a request for the text (not just the list of formats, and not from the window manager's own clipboard keeper) trips a warning. XFixes reports the next clipboard owner change. The text is read once, then KeyHandoff takes the clipboard and refuses every paste until something else is copied. X11 has no shared history API, so a history manager may already have stored the copy. Wayland isn't supported.

## Programs asking for keys

A `.env` pointer such as `DIALPAD_API_KEY=handoff:Dialpad` names a kept key. A loader sends one line of JSON, `{"names": ["Dialpad"]}`, and gets one line back: `{"values": {"Dialpad": "…"}}` or `{"error": "…"}`.

- **Linux:** a `0600` Unix socket, `handoff-<profile>-keys.sock`, in the runtime directory. `SO_PEERCRED` gives the asking process, which must be this user; `/proc` gives its executable and working folder.
- **Windows:** a local named pipe, `\\.\pipe\handoff-<user>-<profile>-keys`, with remote clients rejected. `GetNamedPipeClientProcessId` gives the process; its image path and its working folder (from the process parameters in its PEB) identify it.

An approval is a program path, a folder and a key id, kept in `approvals.bin`. A request with any key not yet allowed waits up to 60 seconds for an answer in the window, and one program at a time can wait. Forgetting a key removes its approvals, and a key kept once is destroyed after it's given out. The reply is built in a buffer sized so it never reallocates, and zeroed after it's sent.

## Transport: .handoff files

Requests and sealed keys are `.handoff` files, and the installer registers the file type.

- **File contents are armored text:** a plain-language line, BEGIN and END markers, Crockford base32 and Reed-Solomon parity, so rewrapped lines, quote markers, curly quotes and some damaged characters are repaired. A renamed or mangled file still opens.
- **Copy** puts the file on the clipboard, with the armored text as a fallback for chats that only take text. This is ciphertext, never a plain key.
- **Receiving** accepts a double-click, a dropped file or a paste. Rust reads the clipboard for a paste.
- A second launch passes its files to the running instance over a local named pipe on Windows, or a Unix socket on Linux.

There are no links and no website. An https link would need a domain whose hijacking could swap a request's lock.

## Testing

- `crates/seal` and the vault: `cargo test`, with synthetic secrets only.
- `src-tauri`: `cargo test` runs the vault, store and whole-flow tests between two identities with a fake clipboard. `cargo test -- --ignored --test-threads=1` adds the real clipboard checks.
- Never use real keys in tests or fixtures.
