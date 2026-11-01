# What KeyHandoff protects, and what it doesn't

KeyHandoff is pre-release and has not had a security audit. This page lists what holds as written, and the gaps, so nobody has to find them on their own. Each claim links to the test that checks it or the design rule that requires it. Rules are in [docs/rules/keyhandoff.md](rules/keyhandoff.md).

## What holds

### In transit

- **Only the requester's computer can open a sealed key.** Another computer, even one with a pending request of its own, can't decrypt it. Test: [`wrong_recipient`](../crates/seal/tests/seal.rs).
- **A sealed key opens once.** Opening it destroys the request's one-time key, so a second copy of the same file, in the chat or anywhere else, can't be opened by anyone. This still holds after a restart. Tests: [`opened_after_destroy`](../crates/seal/tests/seal.rs), [`a_card_lets_ben_send_ann_saved_keys_each_opening_once`](../src-tauri/src/core.rs). Rule 5.
- **Requests run out.** A sender can't seal to an expired request, and a key sealed in time but opened late is refused. Test: [`expired_request`](../crates/seal/tests/seal.rs).
- **Changed bytes are caught.** Changing any byte of a request or a sealed key fails its signature, so a swapped lock in a request is detected. Test: [`tampering_is_detected`](../crates/seal/tests/seal.rs).
- **Damage is repaired or refused, never misread.** Text mangled by a chat app is repaired within a budget. Beyond it, the file is refused rather than opened as something else. Tests: [`survives_transit_damage`](../crates/seal/tests/armor.rs), [`too_damaged_is_refused_not_misread`](../crates/seal/tests/armor.rs).

### On the sender's computer

- **Sending leaves nothing new behind.** You send a key you already hold; KeyHandoff seals it from your vault without the clipboard, and closing the window deletes the sealed file. Test: [`a_card_lets_ben_send_ann_saved_keys_each_opening_once`](../src-tauri/src/core.rs). Rule 6.

### On the receiver's computer

- **Kept keys are encrypted on disk.** No key value appears in the vault or any other profile file, and a different master key can't open them. Tests: [`protected_files_hold_no_plaintext_and_need_the_key`](../src-tauri/src/store.rs), [`stores_sealed_and_round_trips`](../src-tauri/src/vault.rs).
- **Each kept key is bound to its entry.** Moving one key's ciphertext onto another entry makes it fail to open. Test: [`swapped_ciphertext_does_not_open`](../src-tauri/src/vault.rs).
- **Keys aren't shown.** The screens get names, states and ids only, never a value, and there's no way to reveal a key in normal use. Test: [`a_card_lets_ben_send_ann_saved_keys_each_opening_once`](../src-tauri/src/core.rs) checks that no value reaches the screens. Rules 6 and 8.
- **Secret values aren't printed in logs.** Debug output of a payload never includes the secret. Test: [`debug_never_prints_the_secret`](../crates/seal/tests/seal.rs).
- **No servers, accounts or telemetry.** The only network traffic is the update check below. Rule 6.
- **Updates are signed, and nothing else installs.** An installed copy asks GitHub for the latest release's `latest.json` at launch and every six hours, and downloads a newer package. The updater installs it only if its signature checks out against the release key's public half, built into the app; a changed byte fails. Packages are signed in CI with a key kept as a GitHub Actions secret. Code: [`update.rs`](../src-tauri/src/update.rs). Rule 6.
- **The update check shows GitHub that you run KeyHandoff.** GitHub sees your IP address and fetches the same file for everyone; nothing about you, your keys or your contacts is sent. Whoever holds the release key, and the GitHub account that publishes with it, can ship code to every installed copy: that's the price of fixing a flaw everywhere quickly. Copies built from source never check.

### When a program uses a key

- **A `.env` file holds a pointer, not the key.** A line like `DIALPAD_API_KEY=handoff:Dialpad` is all that's written in the project, and a pointer names exactly one key. Tests: [`loader_gets_the_key_through_the_socket`](../src-tauri/src/core.rs) checks, for the Python and Node loaders, that the project folder doesn't hold the key, and [`names_are_unique_so_each_pointer_finds_its_own_key`](../src-tauri/src/vault.rs).
- **A program gets a key only after you allow it.** The approval covers one command (the program, its arguments and the contents of every file they name), in one folder, for one key. An edited script, the same interpreter running other code (`python -c …`), the same program in another folder, or another program has to ask, and Deny gives it nothing. Tests: [`programs_get_only_the_keys_they_were_allowed_in_that_folder`](../src-tauri/src/core.rs), [`keyhandoff_run_binds_approval_to_the_command_names_agents_and_masks_output`](../src-tauri/src/core.rs).
- **Under `keyhandoff run`, the approved command is the command that runs.** The runner names the exact command it will start, and KeyHandoff believes that only from its own executable (the same file, or one with the same contents); any other program claiming to be a run is refused. Same test.
- **A command an AI coding agent started always asks.** If Claude Code, Cursor, Codex, GitHub Copilot, Aider, Windsurf or Gemini CLI is among the processes that started it (`/proc` on Linux, a Toolhelp snapshot on Windows), or Claude Code's `CLAUDECODE=1` marker is in its environment, saved approvals don't count: the window names the agent and the exact command, and offers 30 minutes, which is never saved, instead of Always. Same test, with a process named `claude` in the chain.
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

- **A program that has a key can do anything with it,** including print it, save it or send it somewhere. With a loader, or a key without a server under `keyhandoff run`, KeyHandoff's part ends when the value reaches the program's environment.
- **The prompt is a decision, not a lock.** It names the command and the folder, but anything running as you could run that same command in that same folder and get the key. Rule 4.
- **A loader's command is what the program says it is.** Under `keyhandoff run` the runner starts exactly the approved command. A program using a loader reports its own command line, and a program running as you can change what it reports, so for loaders the binding stops an honest mistake (an edited script, a one-off `python -c`), not a program built to imitate an approved one. Rule 4.
- **Agent detection is a warning, not a promise.** It goes by process names; an agent that runs under another name, or a command that detaches from the agent's process tree, isn't recognised and gets the ordinary prompt. Binding the approval to the command is what holds either way. Rule 4.
- **Masking catches keys printed by accident, not ones smuggled out.** Under `keyhandoff run`, what the command and everything it starts write to stdout and stderr passes through KeyHandoff, and a key printed plain, base64, hex, URL-encoded or reversed, or any run of 12 or more of its characters in one of those forms, shows as `[KeyHandoff: Stripe hidden]`, however the bytes are split. The Python loader covers its process's stdout and stderr the same way, including programs it starts; the Node.js loader covers `process.stdout` and `process.stderr` only. A key written to a file or the network, sent to the terminal directly (`/dev/tty`), or encoded some other way isn't caught. Tests: [`every_printed_form_is_hidden_whatever_the_chunking`](../src-tauri/src/mask.rs), [`keyhandoff_run_binds_approval_to_the_command_names_agents_and_masks_output`](../src-tauri/src/core.rs), [`loader_gets_the_key_through_the_socket`](../src-tauri/src/core.rs). Rule 4.
- **A key without a server starts the program with the key in its environment.** Under `keyhandoff run`, a key whose servers aren't known (not set on the key, and not a prefix KeyHandoff recognises) goes to the program itself, and while it runs other programs running as you can read a process's starting environment (`/proc/<pid>/environ` on Linux, its PEB on Windows). On Windows every key goes this way until the schannel hook is built. Set the key's server to get a placeholder instead.
- **Python and JavaScript strings can't be wiped,** so the value stays in the program's memory until it exits. See [loaders/](../loaders/).

### Placeholders under `keyhandoff run`

When a key's servers are known (**Only for api.stripe.com** on its row, filled in from the prefix of well-known keys or typed in), a program started by `keyhandoff run` on Linux gets a placeholder: the key's prefix and length with random characters after it, fresh for every run. KeyHandoff's TLS hook, preloaded into the program, swaps the real key in at the moment an outgoing TLS write is encrypted.

- **The key reaches only its own server, and only inside encryption.** The hook swaps it in only on a connection whose certificate chain verifies against the system's trusted roots (plus a CA set on the key for a private server) and is valid for one of the key's servers. It doesn't take the program's word: not its trust settings, not `SSL_CERT_FILE`, not the name it asked for in SNI, not a `Host` header. Test: [`keyhandoff_run_swaps_the_key_in_only_for_its_verified_host`](../src-tauri/src/core.rs): the key's server gets the key; a server with a valid certificate for another name, the same server named as the key's host in SNI, and a self-signed certificate for the key's host get nothing.
- **The program's environment, files and logs hold only the placeholder.** Same test: `/proc/self/environ` and the variable the program reads hold no key. A stolen placeholder is worth nothing once the run ends: KeyHandoff stops swapping it then, and every run gets a new one.
- **Only that run's processes can swap.** The hook asks KeyHandoff with a token it removes from the environment before the program's own code starts and passes on to programs it starts; KeyHandoff answers only processes started by that run's runner, and only while the runner is connected. Locking KeyHandoff ends every run.
- **Code inside the program can still use the key, and can get it out by attacking the hook.** Code running in the program (a malicious package, say) can make requests to the key's server as you, as any program allowed the key could. And the key is in the program's memory for the moment of each swap, so code written to attack KeyHandoff's hook specifically, by reading that memory or by asking KeyHandoff the way the hook does, can take it. What it stops is the common case: code that reads the environment or `.env`, prints, logs, uploads or pastes the key gets a placeholder. Rule 4.
- **Coverage is the program's TLS library.** The hook interposes OpenSSL's `SSL_write` and `SSL_write_ex` when OpenSSL is loaded as a shared library: Python, Ruby, PHP, curl, `psql` and most Linux tools. Node.js links its own OpenSSL, so `keyhandoff run` also has Node load a hook of its own (`src-tauri/hook/keyhandoff_node.cjs`, through `NODE_OPTIONS`) that wraps TLS sockets' writes and checks the server's chain itself against Node's root store and the key's CA; Node's HTTP/2 module isn't covered. Go, Rust with rustls, Java and programs that link OpenSSL statically use their own TLS, so their placeholder goes out unchanged and the server refuses it: safe, but the request fails, and it isn't caught going elsewhere. A key a request signs rather than sends (AWS) can't work this way, so KeyHandoff doesn't guess a server for it.
- **HTTP/2 headers are swapped too.** HTTP/2 compresses headers (HPACK, usually Huffman-coded) before they reach `SSL_write`. On a connection that chose HTTP/2, the hook follows its frames from the connection preface, decodes each complete header block, swaps the placeholder where the connection allows and encodes the block again the same way; a placeholder headed elsewhere in a header block is caught the same way. A header block split over CONTINUATION frames or across writes isn't read. The Huffman table is measured from the system's libnghttp2 and checked against it (`src-tauri/hook/gen_hpack_huffman.py`).
- **Other encodings aren't seen.** A placeholder in a compressed body, a WebSocket message (clients mask every byte), or split across two writes goes out as the placeholder: the request fails, and it isn't caught going elsewhere. HTTP/3 doesn't use `SSL_write` at all.
- **The coverage matrix** ([eval/results/coverage.md](../eval/results/coverage.md)) runs the same keyed request through each client and protocol, to the key's server and to a stranger, and CI fails if a claimed cell isn't protected or a stranger ever gets the key.

### Tripwires

Nothing legitimate sends a placeholder anywhere but its key's servers, so a placeholder headed elsewhere is a leak in the act.

- **A placeholder going elsewhere is stopped before a byte leaves.** Over TLS, the write that holds it is refused when the connection isn't verified as one of the key's servers. Over a plain internet socket (`send`, `sendto`, `sendmsg`, `write`, `writev`), any write that holds it is refused. The program sees a refused connection. Tests: [`keyhandoff_run_swaps_the_key_in_only_for_its_verified_host`](../src-tauri/src/core.rs), [`a_leaked_placeholder_is_blocked_traced_and_refused_for_the_team`](../src-tauri/src/core.rs).
- **Every run gets its own placeholders, and KeyHandoff remembers them.** A placeholder found later, in a log, a commit, a pasted message or an AI transcript, says which key, which command and when: paste the text into KeyHandoff. Same test.
- **The alert names the run and the destination,** and KeyHandoff keeps it. Same test.
- **The destination is refused from then on,** for every program you start with `keyhandoff run`: TLS connections whose name or certificate matches it, plain connections to its address, and looking up its name. Same test.
- **The team is warned.** KeyHandoff writes a signed warning (`warnings/` in each team folder you're on) naming the destination and the command. Teammates' KeyHandoffs refuse that destination for their own programs, taking warnings only from the keeper and members of the team's latest version. A warning is signed, not encrypted: anyone who can read the folder can read the destination and command. Test: [`a_leaked_placeholder_is_blocked_traced_and_refused_for_the_team`](../src-tauri/src/core.rs), [`a_warning_verifies_and_a_changed_byte_does_not`](../crates/seal/src/warning.rs).
- **A placeholder encoded or split before it's sent isn't recognised.** The tripwire looks for the placeholder as written; base64 of it, or half of it, goes out. It's still only a placeholder, and that run's ends with the run. The same goes for a placeholder in a DNS name lookup that isn't a plain copy of it, or one sent by a program the hook isn't in (see coverage above). Rule 4.
- **Refusing a destination can be too broad or too narrow.** A name is refused by name, so a leak to a shared service (a paste site, a cloud storage host) refuses that whole service for your runs; and an attacker can use a new name next time. Rule 4.
- **Anyone on the team can refuse a destination for everyone.** A teammate's computer, or malware on it, could warn about a service you need. The warning names who wrote it.

### Anything running as you, or as an administrator

- **The vault is locked by your passphrase, not by hardware.** While KeyHandoff is locked or closed, a program running as you can read every profile file and still can't open a key without the passphrase or recovery code. Test: [`only_the_passphrase_or_recovery_code_opens_the_vault`](../src-tauri/src/store.rs). But a copy of the files can be attacked offline by guessing passphrases; Argon2id makes each guess slow, and five random words are out of reach, but a short or common passphrase isn't. A TPM PIN, where the computer has a TPM, rate-limits guessing; Windows Hello is planned. KeyHandoff only uses hardware the computer already has. See [architecture.md](architecture.md#key-storage).
- **While it's unlocked, the vault is open to anything running as you.** A program could read KeyHandoff's memory or ask for keys the way an allowed program does. It locks again when the computer sleeps, when the screen locks (Linux: logind's `LockedHint`; Windows: the session's lock state) and from the lock button in the title bar, and after a quiet spell if you pick Lock after under Security (saved as `lock_after_minutes` in `settings.json`). Locking drops the master key and everything opened with it. Test: [`idle_locks_and_drops_the_key_until_unlock`](../src-tauri/src/session.rs).
- **A TPM PIN can be short because the TPM counts wrong tries.** Someone with a copy of `keyslot-tpm.bin` can't use it on another computer, and on this one the TPM locks guessers out after a few wrong PINs. Test: [`a_tpm_pin_opens_the_vault_on_this_tpm_only`](../src-tauri/src/store.rs), against the swtpm software TPM. The PIN reaches the TPM in a password session, so someone with a probe on the board of a computer with a separate TPM chip could read it; that's outside what KeyHandoff guards against.
- **"Remember on this computer" gives that up, on purpose.** With it on, a copy of the master key sits with the OS (DPAPI, or the Secret Service or a `0600` file on Linux) and any program running as you can use it to open the vault. It's off unless you turn it on, and the Security sheet says so before you do. Test: [`remembering_opens_without_the_passphrase_until_turned_off`](../src-tauri/src/store.rs).
- **The passphrase is typed into KeyHandoff's window,** so a keylogger running as you can record it. On X11 any program can read the keyboard.
- **Memory isn't locked yet.** A key being used is decrypted in KeyHandoff's memory, which could reach the page file or a crash dump.
- **An administrator, or malware running as one, can do everything above and more:** read KeyHandoff's memory while a key is in use, record the keyboard and screen, watch the clipboard, and replace KeyHandoff itself. No app can protect a secret on a computer an attacker controls.

### Cards and teams

- **Each lock opens one key, once.** A card is a batch of one-time locks; every key sent with it uses one, and opening it destroys that lock. Tests: [`every_lock_seals_one_key_that_opens_once`](../crates/seal/tests/card.rs), [`a_card_lets_ben_send_ann_saved_keys_each_opening_once`](../src-tauri/src/core.rs).
- **A card copy is for one person.** If one copy reaches several people, two of them could use the same lock, and the second key wouldn't open. Each copy gets fresh locks, and senders pick a lock at random to make a clash rare, but it's not impossible. Rule 4.
- **Teams live on each computer.** A team is a list of your contacts, and the team name inside a file isn't signed by anyone. Sending to a team seals a separate copy for each member, and you keep one too. Test: [`a_team_gets_each_key_once_and_a_newcomer_gets_them_all`](../src-tauri/src/core.rs). Rule 6.
- **No one can be removed after the fact.** Taking someone off your team stops you sending them new keys, but the keys they already have stay with them. When someone leaves, replace the keys with the provider.

### Nearby

- **Off until you turn it on, and only while unlocked.** Locking stops it: nothing listens or announces. Test: [`nearby_takes_keys_only_from_people_on_your_list_and_stops_when_locked`](../src-tauri/src/core.rs). Rule 6.
- **Only people on your list can hand you a key.** A key sealed by anyone else is refused, even someone who has your card. Same test.
- **The network only carries sealed keys.** Someone else on the network sees that two KeyHandoffs talked, never a key.
- **What it announces.** While it's on, this computer's fingerprint is visible to everything on the local network, so someone there could tell the same computer came back. On a network you don't trust, leave Nearby off.
- **Pairing in person is checked by a code.** Both people choose Add someone nearby, swap cards, and compare a six-digit code worked out from both cards. Someone on the network who swaps a card in transit makes the two screens show different codes, so nobody is added unless both people confirm a match. A card that arrives when you aren't adding anyone is refused. Tests: [`pairing_nearby_swaps_cards_and_both_see_the_same_code`](../src-tauri/src/core.rs), [`both_sides_get_one_code_and_a_swapped_card_changes_it`](../src-tauri/src/nearby.rs). A six-digit code makes a lucky swap one in a million; read it carefully.
- **Flooding is bounded.** At most 8 connections are handled at once, each has one 5-second deadline for the whole exchange however slowly bytes arrive, memory grows only as data actually arrives, and at most 64 computers are remembered from announcements. Someone flooding the network can delay a send; they can't wear the computer down. Test: [`a_flood_of_slow_connections_is_bounded_and_cut_off`](../src-tauri/src/nearby.rs).
- **A fake announcement loses a key, it doesn't leak one.** If someone on the network claims to be Ann's computer, a key sent "nearby" goes to them still sealed for Ann, and only Ann's computer can open it. Ann just doesn't get it; send the file instead.

### People and channels

- **First contact trusts the channel.** If someone can post as you in a chat, they can send a request in your name. Check the name and the computer fingerprint KeyHandoff shows before you copy a key.
- **No central revocation, and no recovery.** Keys are tied to one computer. If a computer is lost or you suspect a leak, replace the key with the provider.

## Reporting a problem

See [SECURITY.md](../SECURITY.md). Please don't include real keys in a report.
