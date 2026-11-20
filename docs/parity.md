# What works on which system

KeyHandoff aims to do the same things, with the same protections, on every system it runs on ([ADR 0006](adr/0006-one-core-thin-os-layers.md)). This table is the one place that says where that isn't true yet. Every gap names the issue that closes it.

- **yes**: built, and tested on that system.
- **partly (ID)** or **no (ID)**: missing or incomplete there; the issue closes it.
- **n/a**: the system has no such thing; the note says why.

Linux means X11. Wayland isn't supported yet on any row. macOS isn't built yet (FEAT-22), so its column is empty for now.

`scripts/check-parity-doc.sh` checks this table in CI: every partly or no cell must name an issue, and where the Ishoo store is present, that issue must still be open.

## Keeping and sending keys

| Feature | Windows | Linux | macOS | Notes |
|---|---|---|---|---|
| Vault, passphrase and recovery code | yes | yes | no (FEAT-22) | Shared code. |
| Backup and restore | yes | yes | no (FEAT-22) | Shared code. |
| TPM PIN unlock | no (PAR-05) | yes | no (FEAT-22) | Linux uses `/dev/tpmrm0`. Windows needs TBS, or Windows Hello in its place (HAND-77). |
| Remember on this computer | yes | yes | no (FEAT-22) | DPAPI on Windows; the Secret Service, or a `0600` file, on Linux. |
| Locks on sleep and screen lock | yes | yes | no (FEAT-22) | The session's lock state on Windows; logind's `LockedHint` on Linux. |
| Sealed `.handoff` files, cards and contacts | yes | yes | no (FEAT-22) | Shared code. |
| Nearby (same network) | yes | yes | no (FEAT-22) | Shared code. |
| Team folders and warnings | yes | yes | no (FEAT-22) | Shared code. |
| Opening `.handoff` files from the file manager | yes | yes | no (FEAT-22) | |
| Drag a file out, paste one in | yes | yes | no (FEAT-22) | |

## Safe Copy

| Feature | Windows | Linux | macOS | Notes |
|---|---|---|---|---|
| Catch one copied key, then wipe the clipboard | yes | yes | no (FEAT-22) | |
| Remove the copy from clipboard history | yes | n/a | no (FEAT-22) | Windows deletes the Win+V entry. X11 has no shared history; a history manager may already have kept it, and Safe Copy names known ones. |
| Warn when another program reads the clipboard while listening | no (HAND-66) | yes | no (FEAT-22) | Linux serves bait text and sees who asks. |
| Name known clipboard managers | yes | yes | no (FEAT-22) | |

## Programs asking for keys

| Feature | Windows | Linux | macOS | Notes |
|---|---|---|---|---|
| Loaders and `keyhandoff run` | yes | yes | no (FEAT-22) | A named pipe on Windows, a Unix socket on Linux. |
| Approvals bound to the exact command and its files | yes | yes | no (FEAT-22) | Shared code. |
| AI coding agents recognised, saved approvals don't count | yes | yes | no (FEAT-22) | |
| Approve for one agent session | yes | yes | no (FEAT-22) | |
| Unattended jobs (services, scheduled tasks, cron, systemd) always ask | yes | yes | no (FEAT-22) | Windows: services, Task Scheduler, WMI. Linux: cron, at, systemd system and user services and timers. |
| Lasting approvals confirmed by the OS's own prompt | no (FEAT-19) | no (PAR-06) | no (FEAT-22) | Windows Hello on Windows; polkit on Linux. |
| Type a saved key into a field | no (HAND-14) | yes | no (FEAT-22) | X11 only. |

## Stand-ins under `keyhandoff run`

| Program | Windows | Linux | macOS | Notes |
|---|---|---|---|---|
| Node.js | yes | yes | no (FEAT-22) | The JavaScript hook, on both. |
| Python, Ruby, PHP, curl (OpenSSL) | no (PAR-08) | yes | no (FEAT-22) | On Windows these get the real key, with printed copies masked. |
| Go, Java, Rust with rustls | no (FEAT-18) | no (FEAT-18) | no (FEAT-22) | They bring their own TLS; the planned proxy covers them. |
| Tripwire: a stand-in sent anywhere else is stopped | partly (PAR-08) | yes | no (FEAT-22) | On Windows, only for Node.js. |

## Updates and testing

| Feature | Windows | Linux | macOS | Notes |
|---|---|---|---|---|
| Signed updates, held 48 hours | yes | yes | no (FEAT-22) | A `.deb` update waits for **Restart to update**, since installing asks for your password. |
| Attack, coverage and tripwire suites run in CI | no (PAR-02) | yes | no (FEAT-22) | |
| Every other test runs in CI | yes | yes | no (FEAT-22) | |
