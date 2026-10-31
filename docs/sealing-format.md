# Sealing format and device key storage

**Status:** Implemented in `crates/seal` as format v0. Pre-release: a breaking change bumps the version byte.



## Problem

Sealing, requests and the vault can't be built until the primitives, the file format and Windows key protection are chosen. The choice must:

- use only standard, audited primitives;
- make a sealed file unopenable once its one-time key is destroyed, whether it was accepted or the request expired;
- fit in a few hundred characters of pasted text;
- live in a standalone crate with no UI or OS dependencies, so it can be reviewed on its own.

## Decision

1. **Primitives** (RustCrypto and dalek crates): **Ed25519** device identity for signatures, **X25519** for the one-time and ephemeral keys, **HKDF-SHA256** for key derivation, **ChaCha20-Poly1305** as the AEAD. Randomness comes from the OS (`getrandom`).
2. **Pairing shape:** ECDH, then HKDF salted with a hash of the request context, then an AEAD with record-bound associated data. Requests travel through chat rather than an authenticated server, so they are also signed.
3. **File format v0**, binary inside armored text (layout below). Every object starts with `HOF` + version `0` + kind.
4. **Key protection: a passphrase** (Argon2id) wraps the profile's master key, which seals the identity, the pending one-time keys and the vault key; a recovery code is a second way in. This replaced DPAPI, which any process running as the user can unwrap. Hardware slots (TPM PIN, Windows Hello, security key) are planned.
5. **Armored text:** Crockford base32 in groups of four, Reed-Solomon parity over GF(2^8) with interleaving, and an integrity check that rejects anything repaired wrongly.

## Sealing in detail

### Identity

Each computer generates one Ed25519 key pair on first launch. Its public half is the computer's identity. Other computers remember it per display name for continuity warnings.

### Request (kind 1)

| Field | Bytes | Notes |
|---|---|---|
| magic | 3 | `HOF` |
| version | 1 | `0` |
| kind | 1 | `1` = request |
| request id | 16 | random |
| created at | 8 | Unix seconds, big-endian |
| expires at | 8 | Unix seconds; default created + 7 days |
| requester identity | 32 | Ed25519 public key |
| one-time key | 32 | X25519 public key, fresh per request |
| name length + name | 1 + n | UTF-8, 1 to 64 bytes, no control characters |
| signature | 64 | Ed25519 by the requester identity over `handoff:request:v0` and every byte above |

About 170 bytes plus the name. It holds no secret.

The requester keeps a **pending entry**: request id, X25519 one-time private key, expiry and SHA-256 of the whole signed request.

### Sealed key (kind 2)

The sender checks the request's signature and expiry, then:

1. Generates an ephemeral X25519 key pair.
2. `shared = X25519(ephemeral, one-time public)`, rejecting an all-zero result (low-order point).
3. `key = HKDF-SHA256(salt = SHA-256(signed request), ikm = shared, info = "handoff:seal:v0" ‖ ephemeral public ‖ one-time public)`. The salt binds the key to that exact request, including the requester's identity and name.
4. Encrypts the payload with ChaCha20-Poly1305 under a random 12-byte nonce. The associated data is every header byte before the nonce.
5. Signs the whole object with the sender's identity, which lets the receiver warn if a known sender's key changes.

| Field | Bytes | Notes |
|---|---|---|
| magic, version, kind | 5 | kind `2` = sealed key |
| request id | 16 | which pending entry opens it |
| ephemeral key | 32 | X25519 public key |
| sender identity | 32 | Ed25519 public key |
| nonce | 12 | random |
| ciphertext length + ciphertext | 2 + n | payload plus a 16-byte tag |
| signature | 64 | Ed25519 by the sender over `handoff:sealed:v0` and every byte above |

The encrypted **payload** holds a label (for example "Dialpad", up to 64 bytes), an optional sender note (up to 200 bytes) and the secret (1 to 4096 bytes). The label sits inside the encryption, so the file shows nothing about what it carries.

### Card (kind 3)

A batch of requests from one computer, so someone can seal several keys to you without asking each time.

| Field | Bytes | Meaning |
|---|---|---|
| header | 5 | `HOF`, version 0, kind 3 |
| count | 1 | 1 to 32 locks |
| locks | count × (2 + n) | each a big-endian length and a complete request (kind 1), signed as usual |

Every lock must come from the same identity under the same name, or the card is refused. Each copy of your card gets fresh locks, and the pending one-time keys for all of them go into the keyring with a 180-day lifetime. A sender spends one lock per key, chosen at random, so two people holding the same copy rarely collide.

### Bundle (kind 4)

Several sealed keys in one file, labelled with a team name: one key sealed to each member of a team, or every team key sealed to one newcomer.

| Field | Bytes | Meaning |
|---|---|---|
| header | 5 | `HOF`, version 0, kind 4 |
| team | 1 + n | name length and UTF-8 name, 1 to 64 bytes |
| count | 1 | 1 to 200 parts |
| parts | count × (2 + n) | each a big-endian length and a complete sealed key (kind 2) |

Each part is verified and opened like a sealed key; a recipient opens the parts sealed to its own locks and skips the rest. The team name is a label, not signed by anyone.

### Open and one-time key lifecycle

- **Open:** find the pending entry by request id; check the expiry; verify the sender's signature; derive the key; decrypt. On success, **destroy the pending entry** (zeroized and removed from storage) before the secret goes to the vault. Every copy of the sealed file is then permanently unopenable.
- **Expiry:** the app purges expired pending entries at launch and hourly. Open also refuses an expired entry even if the purge hasn't run yet.
- **Cancel:** the user can withdraw a request, which destroys its entry.
- **Opening twice** or opening after expiry gives the same message: "This key was already opened, or the request ran out. Ask again."

### Lost keys

- **Lost or wiped computer:** its identity, pending entries and vault are gone. Sealed files still in transit can never be opened. The person installs KeyHandoff again and asks again; people who dealt with the old computer see a continuity warning, which is expected and is explained by the warning text.
- **Pending entry lost before open:** the sender kept nothing, so they have to safe-copy the key again, which may mean making a new key with the provider.
- **No recovery or escrow.** Rotating with the provider is the real fix .

## Device key storage

- **Current:** one 32-byte master key per profile, stored only in `keyslots.bin` (`src-tauri/src/slots.rs`): `HOFK`, version 0, a slot count, then per slot its kind (1 passphrase, 2 recovery code), Argon2id memory in KiB and passes (u32 LE), lanes, a 16-byte salt, a 12-byte nonce and the 48-byte ChaCha20-Poly1305 sealed master key, with everything before it as associated data. `identity.bin`, `pending.bin` and `vault.bin` are sealed under the master key with the file name as associated data, and each kept item is sealed again under the vault key with its id (`store.rs`, `vault.rs`).
- **Honest limit:** a copy of the folder can be attacked offline by guessing the passphrase, so its strength matters; a suggested passphrase is five words from the EFF long list (about 64 bits). Once unlocked, the key is in KeyHandoff's memory until it quits.
- **Before key slots:** DPAPI (Windows) or the Secret Service or a `0600` file (Linux) held the master key and released it to any process running as the user. Those profiles move into slots when the passphrase is first set, and the old copy is deleted.
- **Planned:** more slot kinds, each wrapping the same master key: a TPM 2.0 key behind a PIN, Windows Hello signing a stored challenge, and a FIDO2 security key's hmac-secret. The rest of the format doesn't change.

## Armored text

How a `.handoff` file looks, and what someone pastes when a channel can't carry files:

```
Ann is asking for a key. Open this with KeyHandoff.
-----BEGIN HANDOFF REQUEST-----
HRQ1 7KQ4 M2PD 9XWR TH3C K8N5
...
-----END HANDOFF REQUEST-----
```

- **Plain-language line** above BEGIN. The reader ignores it.
- **Markers:** `BEGIN HANDOFF REQUEST` or `BEGIN HANDOFF SEALED KEY`, found case-insensitively anywhere on a line, so chat apps that turn dashes into long dashes don't break it. A missing END means "read to the end".
- **Body:** Crockford base32 (no I, L, O or U; on reading, O becomes 0 and I or L becomes 1), groups of 4 characters, 6 groups per line. On reading, quote markers (`>`), curly quotes, hyphens and extra whitespace are dropped, and lines may be rewrapped.
- **Damage:** a group with an invalid character, or with too few or too many characters, is treated as missing (an erasure) rather than guessed at. Reed-Solomon repairs erasures at half the cost of unknown errors.
- **Inside the body:** a 10-byte preamble (armor version, payload length, 7 parity bytes) and then the payload plus a 4-byte SHA-256 check, split into equal blocks, each with about 30% Reed-Solomon parity, and interleaved so one damaged group touches several blocks lightly instead of one block heavily.
- **Beyond repair:** if the parity can't repair the damage, or a repair doesn't match the check, the reader refuses with "This message is too damaged to open. Ask them to send it again." It never passes on a wrong result.
- **Measured** (`cargo run --release --example damage_probe` in `crates/seal`, 300 trials per row, a 650-character sealed key): 10 random character changes always open; 20 open 99%; 30 open 97%; 40 open 28%; 60 never. No trial ever returned wrong bytes; the rest were refused. A request is about 620 characters, a sealed key with a 100-character secret about 830, including the plain line and markers.

## Alternatives rejected

- **The `age` format and crate:** a good X25519 + ChaCha20-Poly1305 design, but it has no signatures, no notion of a one-time key lifecycle, and its own armor that doesn't survive chat. The primitives match; the container doesn't.
- **P-256 ECDH + AES-GCM:** fine, but in Rust the dalek X25519 and Ed25519 crates are smaller and harder to misuse.
- **Unsigned requests and sealed keys:** anyone who saw a request in a chat could send a fake key back, or swap the lock under someone's name. Signatures plus continuity make the first contact trust the channel and later contacts checkable.
- **TPM or Windows Hello from the start:** stronger, but slower to build and it can prompt; DPAPI first, same file format.
- **base64 or hex armor:** base64 is mangled by case changes and confusable characters; hex is twice as long. Crockford base32 survives both.

## Consequences

- `crates/seal` depends only on `ed25519-dalek`, `x25519-dalek`, `chacha20poly1305`, `hkdf`, `sha2`, `zeroize` and `rand_core`. No OS or UI crates.
- The KeyHandoff backend owns persistence and DPAPI: it stores the crate's serialized identity and pending entries, wrapped.
- Format v0 is a pre-release format. A breaking change bumps the version byte; v0 readers refuse unknown versions.

## Stable names

The product was originally called Handoff. Files and saved keys depend on these names, so they stay: the `.handoff` extension and `application/x-handoff` type, the `HANDOFF` armor markers, the `handoff:` signing and derivation labels, the key-protection labels and the `handoff` data folder.
