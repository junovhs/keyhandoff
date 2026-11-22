# What works on which system

KeyHandoff aims to do the same things, with the same protections, on every system it runs on ([ADR 0006](adr/0006-one-core-thin-os-layers.md)). This table is the one place that says where that isn't true yet. Every gap names the issue that closes it.

- **yes**: built, and tested on that system.
- **partly (ID)** or **no (ID)**: missing or incomplete there; the issue closes it.
- **n/a**: the system has no such thing; the note says why.

Linux means X11. Wayland isn't supported yet on any row. macOS means 10.15 or later, Intel or Apple silicon, from one universal app; it was built and tested on an Intel Mac running macOS 12.

`scripts/check-parity-doc.sh` checks this table in CI: every partly or no cell must name an issue, and where the Ishoo store is present, that issue must still be open.

## Keeping and sending keys

| Feature | Windows | Linux | macOS | Notes |
|---|---|---|---|---|
| Vault, passphrase and recovery code | yes | yes | yes | Shared code. |
| Backup and restore | yes | yes | yes | Shared code. |
| TPM PIN unlock | no (PAR-05) | yes | no (ISS-14) | Linux uses `/dev/tpmrm0`. Windows needs TBS, or Windows Hello in its place (HAND-77). Macs have no TPM; Touch ID and the Secure Enclave take its place there. |
| Remember on this computer | yes | yes | yes | DPAPI on Windows; the Secret Service, or a `0600` file, on Linux; the login Keychain, or a `0600` file, on macOS. |
| Locks on sleep and screen lock | yes | yes | yes | The session's lock state on Windows; logind's `LockedHint` on Linux; the window server's `CGSSessionScreenIsLocked` on macOS. |
| Sealed `.handoff` files, cards and contacts | yes | yes | yes | Shared code. |
| Nearby (same network) | yes | yes | yes | Shared code. macOS 15 asks once before KeyHandoff uses the local network. |
| Team folders and warnings | yes | yes | yes | Shared code. |
| Opening `.handoff` files from the file manager | yes | yes | partly (ISS-10) | Finder sends files to the running app; built, not yet checked by hand. |
| Drag a file out, paste one in | yes | yes | partly (ISS-10) | Pasting is tested on macOS; dragging out is built, not yet checked by hand. |

## Safe Copy

| Feature | Windows | Linux | macOS | Notes |
|---|---|---|---|---|
| Catch one copied key, then wipe the clipboard | yes | yes | yes | macOS has no clipboard-change notice, so Safe Copy checks the pasteboard every 10 ms, only while it listens. |
| Remove the copy from clipboard history | yes | n/a | no (ISS-16) | Windows deletes the Win+V entry. X11 has no shared history; a history manager may already have kept it, and Safe Copy names known ones. macOS had none before macOS 26, whose Spotlight keeps one. |
| Warn when another program reads the clipboard while listening | no (HAND-66) | yes | no (ISS-15) | Linux serves bait text and sees who asks. |
| Name known clipboard managers | yes | yes | yes | macOS: Paste, Maccy, Alfred, Raycast, Pastebot and others. |

## Programs asking for keys

| Feature | Windows | Linux | macOS | Notes |
|---|---|---|---|---|
| Loaders and `keyhandoff run` | yes | yes | yes | A named pipe on Windows, a Unix socket on Linux and macOS. On macOS the app links the `keyhandoff` command into `/usr/local/bin` when it can. |
| Approvals bound to the exact command and its files | yes | yes | yes | Shared code. |
| AI coding agents recognised, saved approvals don't count | yes | yes | yes | |
| Approve for one agent session | yes | yes | yes | |
| Unattended jobs (services, scheduled tasks, cron, systemd, launchd) always ask | yes | yes | yes | Windows: services, Task Scheduler, WMI. Linux: cron, at, systemd system and user services and timers. macOS: cron, at, launchd agents and daemons. |
| Lasting approvals confirmed by the OS's own prompt | no (FEAT-19) | no (PAR-06) | no (ISS-14) | Windows Hello on Windows; polkit on Linux; Touch ID on macOS. |
| Type a saved key into a field | no (HAND-14) | yes | partly (ISS-10) | X11 only on Linux. macOS uses Accessibility, which you allow once; built, keystrokes not yet checked by hand. |

## Stand-ins under `keyhandoff run`

| Program | Windows | Linux | macOS | Notes |
|---|---|---|---|---|
| Node.js | yes | yes | yes | The JavaScript hook, on all three. |
| Python, Ruby, PHP, curl (OpenSSL) | no (PAR-08) | yes | no (ISS-13) | On Windows and macOS these get the real key, with printed copies masked. macOS strips `DYLD_INSERT_LIBRARIES` from Apple's own programs. |
| Go, Java, Rust with rustls | no (FEAT-18) | no (FEAT-18) | no (FEAT-18) | They bring their own TLS; the planned proxy covers them. |
| Tripwire: a stand-in sent anywhere else is stopped | partly (PAR-08) | yes | partly (ISS-13) | On Windows and macOS, only for Node.js, over TLS or plain TCP. |

## Updates and testing

| Feature | Windows | Linux | macOS | Notes |
|---|---|---|---|---|
| Signed updates, held 48 hours | yes | yes | partly (ISS-17) | A `.deb` update waits for **Restart to update**, since installing asks for your password. Macs update from a signed `.app.tar.gz`; no Mac release has updated to the next one yet. |
| Attack, coverage and tripwire suites run in CI | yes | yes | yes | On Windows and macOS, only Node.js must be protected; other programs show as exposed (PAR-08, ISS-13). |
| Every other test runs in CI | yes | yes | yes | |
