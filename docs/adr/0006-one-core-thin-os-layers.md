# ADR 0006: One shared core, thin per-OS layers

**Status:** accepted
**Date:** 2026-11-20
**Work:** the "Windows and Linux at parity" plan: PAR-01 to PAR-08, HAND-14

## Context

KeyHandoff runs on Windows and Linux (X11), and macOS is planned (FEAT-22). Features have landed on one OS first, with the other side left as a note inside the issue. On 2026-11-20 that left these gaps:

| Works only on | What | Issue |
|---|---|---|
| Linux | Stand-ins for every OpenSSL program; Windows hooks only Node.js | PAR-08 |
| Linux | The attack, coverage and tripwire test suites | PAR-02 |
| Linux | Typing a saved key into a field | HAND-14 |
| Linux | TPM PIN unlock | PAR-05 |
| Windows | Unattended launches (services, scheduled tasks) are treated as unattended | PAR-03 |
| Windows | Approvals scoped to an AI agent's session | PAR-04 |
| Windows (planned) | Lasting approvals confirmed by the OS (Windows Hello) | PAR-06 |

Some of these are protections, not conveniences. On Linux, a cron job asking for a key is treated as a person at the keyboard. On Windows, a Python script gets the real key. A user can't tell which promises hold on their computer, and nobody reviewing the code can either: about 40 `#[cfg(...)]` branches sit in `core.rs`, `run.rs` and `lib.rs`, outside `src/os/`.

The owner wants Windows and Linux at full parity now, with macOS added later for as little new code as possible.

## Decision

1. **Behaviour lives in the shared core.** What KeyHandoff decides (who gets a key, when to ask, what a stand-in is, what counts as a tripwire) is written once. `src-tauri/src/os/<os>/` only adapts one OS facility (clipboard, pipe or socket, process identity, key protection, the OS's own confirmation prompt) to a function declared in `os/mod.rs`. Core code has no `#[cfg]` on the OS. Tests may use one only to pick a fixture. CI enforces this (PAR-01).
2. **Prefer designs that need no per-OS code.** Between two designs, pick the one that works the same everywhere. For example, the service proxy with a name-constrained local CA (ADR 0003, FEAT-18) is preferred over a TLS hook per OS (ADR 0002). Per-OS hooks stay only as a fallback.
3. **A feature is done when it works, and is tested, on Windows and Linux.** If one OS can't have it yet, the gap is a filed issue in the parity plan, and [docs/parity.md](../parity.md) (PAR-07) shows it as missing on that OS with the issue's id. The issue isn't closed until the other side lands.
4. **One test, every OS.** A test of behaviour runs in both CI jobs. Only its fixture differs. Where an OS lacks the feature, the test asserts what that OS does instead, so the gap is visible in CI and not hidden by a `cfg`.
5. **macOS gets a folder.** Adding macOS means writing `os/macos/` against the same interface, plus its fixtures. If it needs changes anywhere else, the interface was wrong.

Some things are OS-only by nature, such as the Windows clipboard history (Win+V) wipe. The parity table marks these "n/a" with the reason, and they don't count as gaps.

## Why

Security promises that hold on one OS and quietly not on the other are worse than fewer promises. Users can't see the difference, and it's the first thing a reviewer or auditor finds. A thin OS layer also keeps the code a reviewer must trust small, and makes macOS cheap. With one maintainer, three copies of every feature can't be kept in step by hand. A rule CI checks can.

## Alternatives rejected

- **Ship each OS's features as they come, and document the gaps.** That's the current state. The gaps stay in issue bodies, and protections drift apart (FIX-08 landed for Windows only).
- **A separate crate per OS.** It adds build and version overhead without making the interface any thinner. Folders under `os/` with one `mod.rs` give the same boundary.
- **Wait for the KeyHandoff service (ADR 0003) and fix parity there.** The service is weeks away. The cron gap (PAR-03) and the Linux-only attack suites (PAR-02) are open today.

## Consequences

- New features take longer to call done, because a Windows-only or Linux-only landing stays open.
- `os/mod.rs` becomes the full list of what KeyHandoff needs from an OS. It should be read before adding a `cfg` anywhere.
- docs/parity.md replaces OS notes scattered through the README and site.
