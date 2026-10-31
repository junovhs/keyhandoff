# What KeyHandoff protects, and what it doesn't

KeyHandoff is pre-release and has not had a security audit. This page lists what holds as written, and the gaps, so nobody has to find them on their own. Each claim links to the test that checks it or the design rule that requires it. Rules are in [docs/rules/keyhandoff.md](rules/keyhandoff.md).

## What holds

### In transit

- **Only the requester's computer can open a sealed key.** Another computer, even one with a pending request of its own, can't decrypt it. Test: [`wrong_recipient`](../crates/seal/tests/seal.rs).
- **A sealed key opens once.** Opening it destroys the request's one-time key, so a second copy of the same file, in the chat or anywhere else, can't be opened by anyone. This still holds after a restart. Tests: [`opened_after_destroy`](../crates/seal/tests/seal.rs), [`request_safe_copy_seal_accept_and_once_only`](../src-tauri/src/core.rs). Rule 5.
- **Requests run out.** A sender can't seal to an expired request, and a key sealed in time but opened late is refused. Test: [`expired_request`](../crates/seal/tests/seal.rs).
- **Changed bytes are caught.** Changing any byte of a request or a sealed key fails its signature, so a swapped lock in a request is detected. Test: [`tampering_is_detected`](../crates/seal/tests/seal.rs).
- **Damage is repaired or refused, never misread.** Text mangled by a chat app is repaired within a budget. Beyond it, the file is refused rather than opened as something else. Tests: [`survives_transit_damage`](../crates/seal/tests/armor.rs), [`too_damaged_is_refused_not_misread`](../crates/seal/tests/armor.rs).

### On the sender's computer

- **The sender keeps nothing.** After sealing, the sender's data folder doesn't hold the key, and closing Safe Copy deletes the sealed file. Test: [`request_safe_copy_seal_accept_and_once_only`](../src-tauri/src/core.rs). Rule 6.

### On the receiver's computer

- **Kept keys are encrypted on disk.** No key value appears in the vault or any other profile file, and a different master key can't open them. Tests: [`protected_files_hold_no_plaintext_and_need_the_key`](../src-tauri/src/store.rs), [`stores_sealed_and_round_trips`](../src-tauri/src/vault.rs).
- **Each kept key is bound to its entry.** Moving one key's ciphertext onto another entry makes it fail to open. Test: [`swapped_ciphertext_does_not_open`](../src-tauri/src/vault.rs).
- **Keys aren't shown.** The screens get names, states and ids only, never a value, and there's no way to reveal a key in normal use. Test: [`request_safe_copy_seal_accept_and_once_only`](../src-tauri/src/core.rs) checks that no value reaches the screens. Rules 6 and 8.
- **Secret values aren't printed in logs.** Debug output of a payload never includes the secret. Test: [`debug_never_prints_the_secret`](../crates/seal/tests/seal.rs).
- **No servers, accounts, telemetry or network code.** Rule 6.

### When a program uses a key

- **A `.env` file holds a pointer, not the key.** A line like `DIALPAD_API_KEY=handoff:Dialpad` is all that's written in the project, and a pointer names exactly one key. Tests: [`loader_gets_the_key_through_the_socket`](../src-tauri/src/core.rs) checks, for the Python and Node loaders, that the project folder doesn't hold the key, and [`names_are_unique_so_each_pointer_finds_its_own_key`](../src-tauri/src/vault.rs).
- **A program gets a key only after you allow it.** The approval covers one program, in one folder, for one key. The same program in another folder, or another program, has to ask, and Deny gives it nothing. Tests: [`programs_get_only_the_keys_they_were_allowed_in_that_folder`](../src-tauri/src/core.rs), [`loader_gets_the_key_through_the_socket`](../src-tauri/src/core.rs).
- **A key kept once is destroyed after its first use.** Test: [`programs_get_only_the_keys_they_were_allowed_in_that_folder`](../src-tauri/src/core.rs).

## What it doesn't protect

### The copy moment

Provider pages show you a key, and you copy it. From that copy until KeyHandoff wipes it, the key is on the system clipboard, unencrypted. Any program watching the clipboard can read it in that moment. Rule 9.

- **On Linux (X11), a program watching the clipboard sees every key you copy.** In 1,000 copies, an independent fast reader saw all 1,000 before Safe Copy took them. X11 hands the copied text to every program that asks, so this isn't a race Safe Copy can win. See [capture-tests.md](capture-tests.md).
- **On Windows this hasn't been measured yet.** Until it has been, treat the moment as long enough for a watching program to win.
- **The bait check is a warning, not a promise.** On Linux, while Safe Copy listens, KeyHandoff puts harmless text on the clipboard; if another program reads it, you're warned before you copy. A program that reads only when a new copy appears, or the desktop's own clipboard keeper (GNOME Shell keeps every copy), won't show up, and Windows has no bait yet. Test: [`x11_bait_trips_only_when_something_reads`](../src-tauri/src/os/linux/clipboard.rs). Rule 4.
- **Safe Copy listens only when you arm it,** takes one copy, then stops, or stops after 60 seconds. Tests: [`x11_capture_takes_one_copy_and_clears_it`](../src-tauri/src/os/linux/clipboard.rs), [`x11_capture_times_out_and_cancels`](../src-tauri/src/os/linux/clipboard.rs), [`catches_one_copy_and_wipes_clipboard_and_history`](../src-tauri/src/os/windows/capture.rs), [`times_out_and_cancels`](../src-tauri/src/os/windows/capture.rs). These use the real clipboard, so they run on request rather than by default. Rule 7.

### Clipboard history

- **Windows:** KeyHandoff deletes the matching entry from clipboard history (Win+V). If it can't delete that one entry, it clears all history and tells you so. Deleting the entry has been checked on its own, but it **hasn't yet been seen working in the real capture flow**, because in tests so far the caught key never reached history to be deleted. Test: [`catches_one_copy_and_wipes_clipboard_and_history`](../src-tauri/src/os/windows/capture.rs). Rule 7.
- **Linux (X11):** there's no shared history API. A clipboard manager may already have stored the copy, and KeyHandoff can't remove it. See [architecture.md](architecture.md#safe-copy-capture).

### Programs you allow

- **A program that has a key can do anything with it,** including print it, save it or send it somewhere. KeyHandoff's part ends when the value reaches the program's environment. A local proxy that adds keys to requests, so scripts never hold them, is planned but not built.
- **The prompt is a decision, not a lock.** It names the program and the folder, but anything running as you could start that same program in that same folder and get the key. Test: [`programs_get_only_the_keys_they_were_allowed_in_that_folder`](../src-tauri/src/core.rs) shows the approval is keyed on program path and folder, which another process running as you can match. Rule 4.
- **Python and JavaScript strings can't be wiped,** so the value stays in the program's memory until it exits. See [loaders/](../loaders/).

### Anything running as you, or as an administrator

- **The vault is locked by your passphrase, not by hardware.** While KeyHandoff is locked or closed, a program running as you can read every profile file and still can't open a key without the passphrase or recovery code. Test: [`only_the_passphrase_or_recovery_code_opens_the_vault`](../src-tauri/src/store.rs). But a copy of the files can be attacked offline by guessing passphrases; Argon2id makes each guess slow, and five random words are out of reach, but a short or common passphrase isn't. Hardware slots (TPM PIN, Windows Hello, security key) that rate-limit guessing are planned. See [architecture.md](architecture.md#key-storage).
- **While it's unlocked, the vault is open to anything running as you.** A program could read KeyHandoff's memory or ask for keys the way an allowed program does. It locks again when the computer sleeps, when the screen locks (Linux: logind's `LockedHint`; Windows: the session's lock state) and from the lock button in the title bar, and after a quiet spell if you set `lock_after_minutes` in `settings.json`. Locking drops the master key and everything opened with it. Test: [`idle_locks_and_drops_the_key_until_unlock`](../src-tauri/src/session.rs).
- **A TPM PIN can be short because the TPM counts wrong tries.** Someone with a copy of `keyslot-tpm.bin` can't use it on another computer, and on this one the TPM locks guessers out after a few wrong PINs. Test: [`a_tpm_pin_opens_the_vault_on_this_tpm_only`](../src-tauri/src/store.rs), against the swtpm software TPM. The PIN reaches the TPM in a password session, so someone with a probe on the board of a computer with a separate TPM chip could read it; that's outside what KeyHandoff guards against.
- **"Remember on this computer" gives that up, on purpose.** With it on, a copy of the master key sits with the OS (DPAPI, or the Secret Service or a `0600` file on Linux) and any program running as you can use it to open the vault. It's off unless you turn it on, and the Security sheet says so before you do. Test: [`remembering_opens_without_the_passphrase_until_turned_off`](../src-tauri/src/store.rs).
- **The passphrase is typed into KeyHandoff's window,** so a keylogger running as you can record it. On X11 any program can read the keyboard.
- **Memory isn't locked yet.** A key being used is decrypted in KeyHandoff's memory, which could reach the page file or a crash dump.
- **An administrator, or malware running as one, can do everything above and more:** read KeyHandoff's memory while a key is in use, record the keyboard and screen, watch the clipboard, and replace KeyHandoff itself. No app can protect a secret on a computer an attacker controls.

### Cards and teams

- **Each lock opens one key, once.** A card is a batch of one-time locks; every key sent with it uses one, and opening it destroys that lock. Tests: [`every_lock_seals_one_key_that_opens_once`](../crates/seal/tests/card.rs), [`a_card_lets_ben_send_ann_keys_each_opening_once`](../src-tauri/src/core.rs).
- **A card copy is for one person.** If one copy reaches several people, two of them could use the same lock, and the second key wouldn't open. Each copy gets fresh locks, and senders pick a lock at random to make a clash rare, but it's not impossible. Rule 4.
- **Teams live on each computer.** A team is a list of your contacts, and the team name inside a file isn't signed by anyone. Sending to a team seals a separate copy for each member, and you keep one too. Test: [`a_team_gets_each_key_once_and_a_newcomer_gets_them_all`](../src-tauri/src/core.rs). Rule 6.
- **No one can be removed after the fact.** Taking someone off your team stops you sending them new keys, but the keys they already have stay with them. When someone leaves, replace the keys with the provider.

### People and channels

- **First contact trusts the channel.** If someone can post as you in a chat, they can send a request in your name. Check the name and the computer fingerprint KeyHandoff shows before you copy a key.
- **No central revocation, and no recovery.** Keys are tied to one computer. If a computer is lost or you suspect a leak, replace the key with the provider.

## Reporting a problem

See [SECURITY.md](../SECURITY.md). Please don't include real keys in a report.
