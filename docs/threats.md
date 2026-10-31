# What KeyHandoff protects, and what it doesn't

KeyHandoff is pre-release and has not had a security audit. This page lists what holds as written, and the gaps, so nobody has to find them on their own. Each claim links to the test that checks it or the design rule that requires it. Rules are in [docs/rules/keyhandoff.md](rules/keyhandoff.md).

## What holds

### In transit

- **Only the requester's computer can open a sealed key.** Another computer, even one with a pending request of its own, can't decrypt it. Test: [`wrong_recipient`](../crates/seal/tests/seal.rs#L43).
- **A sealed key opens once.** Opening it destroys the request's one-time key, so a second copy of the same file, in the chat or anywhere else, can't be opened by anyone. This still holds after a restart. Tests: [`opened_after_destroy`](../crates/seal/tests/seal.rs#L63), [`request_safe_copy_seal_accept_and_once_only`](../src-tauri/src/core.rs#L794). Rule 5.
- **Requests run out.** A sender can't seal to an expired request, and a key sealed in time but opened late is refused. Test: [`expired_request`](../crates/seal/tests/seal.rs#L83).
- **Changed bytes are caught.** Changing any byte of a request or a sealed key fails its signature, so a swapped lock in a request is detected. Test: [`tampering_is_detected`](../crates/seal/tests/seal.rs#L130).
- **Damage is repaired or refused, never misread.** Text mangled by a chat app is repaired within a budget. Beyond it, the file is refused rather than opened as something else. Tests: [`survives_transit_damage`](../crates/seal/tests/armor.rs#L79), [`too_damaged_is_refused_not_misread`](../crates/seal/tests/armor.rs#L160).

### On the sender's computer

- **The sender keeps nothing.** After sealing, the sender's data folder doesn't hold the key, and closing Safe Copy deletes the sealed file. Test: [`request_safe_copy_seal_accept_and_once_only`](../src-tauri/src/core.rs#L794). Rule 6.

### On the receiver's computer

- **Kept keys are encrypted on disk.** No key value appears in the vault or any other profile file, and a different master key can't open them. Tests: [`protected_files_hold_no_plaintext_and_need_the_key`](../src-tauri/src/store.rs#L207), [`stores_sealed_and_round_trips`](../src-tauri/src/vault.rs#L205).
- **Each kept key is bound to its entry.** Moving one key's ciphertext onto another entry makes it fail to open. Test: [`swapped_ciphertext_does_not_open`](../src-tauri/src/vault.rs#L245).
- **Keys aren't shown.** The screens get names, states and ids only, never a value, and there's no way to reveal a key in normal use. Test: [`request_safe_copy_seal_accept_and_once_only`](../src-tauri/src/core.rs#L794) checks that no value reaches the screens. Rules 6 and 8.
- **Secret values aren't printed in logs.** Debug output of a payload never includes the secret. Test: [`debug_never_prints_the_secret`](../crates/seal/tests/seal.rs#L181).
- **No servers, accounts, telemetry or network code.** Rule 6.

### When a program uses a key

- **A `.env` file holds a pointer, not the key.** A line like `DIALPAD_API_KEY=handoff:Dialpad` is all that's written in the project, and a pointer names exactly one key. Tests: [`python_loader_gets_the_key_through_the_socket`](../src-tauri/src/core.rs#L971) checks that the project folder doesn't hold the key, and [`names_are_unique_so_each_pointer_finds_its_own_key`](../src-tauri/src/vault.rs#L226).
- **A program gets a key only after you allow it.** The approval covers one program, in one folder, for one key. The same program in another folder, or another program, has to ask, and Deny gives it nothing. Tests: [`programs_get_only_the_keys_they_were_allowed_in_that_folder`](../src-tauri/src/core.rs#L926), [`python_loader_gets_the_key_through_the_socket`](../src-tauri/src/core.rs#L971).
- **A key kept once is destroyed after its first use.** Test: [`programs_get_only_the_keys_they_were_allowed_in_that_folder`](../src-tauri/src/core.rs#L926).

## What it doesn't protect

### The copy moment

Provider pages show you a key, and you copy it. From that copy until KeyHandoff wipes it, the key is on the system clipboard, unencrypted. Any program watching the clipboard can read it in that moment. Rule 9.

- **How long the moment lasts hasn't been measured yet** against an independent program reading the clipboard as fast as it can. Until it has been, treat the moment as long enough for a watching program to win.
- **Safe Copy listens only when you arm it,** takes one copy, then stops, or stops after 60 seconds. Tests: [`x11_capture_takes_one_copy_and_clears_it`](../src-tauri/src/os/linux/clipboard.rs#L375), [`x11_capture_times_out_and_cancels`](../src-tauri/src/os/linux/clipboard.rs#L397), [`catches_one_copy_and_wipes_clipboard_and_history`](../src-tauri/src/os/windows/capture.rs#L338), [`times_out_and_cancels`](../src-tauri/src/os/windows/capture.rs#L367). These use the real clipboard, so they run on request rather than by default. Rule 7.

### Clipboard history

- **Windows:** KeyHandoff deletes the matching entry from clipboard history (Win+V). If it can't delete that one entry, it clears all history and tells you so. Deleting the entry has been checked on its own, but it **hasn't yet been seen working in the real capture flow**, because in tests so far the caught key never reached history to be deleted. Test: [`catches_one_copy_and_wipes_clipboard_and_history`](../src-tauri/src/os/windows/capture.rs#L338). Rule 7.
- **Linux (X11):** there's no shared history API. A clipboard manager may already have stored the copy, and KeyHandoff can't remove it. See [architecture.md](architecture.md#safe-copy-capture).

### Programs you allow

- **A program that has a key can do anything with it,** including print it, save it or send it somewhere. KeyHandoff's part ends when the value reaches the program's environment. A local proxy that adds keys to requests, so scripts never hold them, is planned but not built.
- **The prompt is a decision, not a lock.** It names the program and the folder, but anything running as you could start that same program in that same folder and get the key. Test: [`programs_get_only_the_keys_they_were_allowed_in_that_folder`](../src-tauri/src/core.rs#L926) shows the approval is keyed on program path and folder, which another process running as you can match. Rule 4.
- **Python strings can't be wiped,** so the value stays in a Python program's memory until it exits. See [loaders/keyhandoff_env.py](../loaders/keyhandoff_env.py).

### Anything running as you, or as an administrator

- **The vault is protected by your sign-in, not by hardware.** On Windows the master key is wrapped with DPAPI, and on Linux it's held by the Secret Service, or a `0600` file if there's none. Both release it to any program running as you. Hardware-backed protection (TPM and Windows Hello) is planned. See [architecture.md](architecture.md#key-storage).
- **Memory isn't locked yet.** A key being used is decrypted in KeyHandoff's memory, which could reach the page file or a crash dump.
- **An administrator, or malware running as one, can do everything above and more:** read KeyHandoff's memory while a key is in use, record the keyboard and screen, watch the clipboard, and replace KeyHandoff itself. No app can protect a secret on a computer an attacker controls.

### People and channels

- **First contact trusts the channel.** If someone can post as you in a chat, they can send a request in your name. Check the name and the computer fingerprint KeyHandoff shows before you copy a key.
- **No central revocation, and no recovery.** Keys are tied to one computer. If a computer is lost or you suspect a leak, replace the key with the provider.

## Reporting a problem

See [SECURITY.md](../SECURITY.md). Please don't include real keys in a report.
