+++
area = "keyhandoff"
becomes = "test:check-platform-code"
date = "2026-11-20"
evidence = ["DOC-02", "HAND-14", "PAR-01", "PAR-02", "PAR-03", "PAR-04", "PAR-05", "PAR-06", "PAR-07", "PAR-08", "base:6bd682521309c4fdea28baabb84fc1d24e692897"]
id = "DEC-05"
revised_at = "2026-11-20T20:51:12.6663559Z"
scope = ["src-tauri/src/", "src-tauri/src/os/", ".github/workflows/check.yml", "docs/parity.md"]
status = "accepted"
superseded_by = []
supersedes = []
title = "One shared core, thin per-OS layers; a feature is done only on Windows and Linux"

[author]
agent = "agent-34504"
person = "Spencer Nunamaker"
+++

# DEC-05: One shared core, thin per-OS layers; a feature is done only on Windows and Linux

## Context

Features landed on one OS first with the other side left as a note: Linux-only stand-ins for non-Node programs, attack suites, typing, TPM PIN; Windows-only unattended detection and agent session grants. About 40 cfg branches sit outside src/os/. The owner wants Windows and Linux at full parity now and macOS later for as little new code as possible (2026-11-20).

## Decision

Behaviour is written once in the shared core; src-tauri/src/os/<os>/ only adapts an OS facility to a function in os/mod.rs, and core code has no OS cfg (CI enforces it, PAR-01). Prefer designs needing no per-OS code (service proxy over per-OS TLS hooks). A feature or protection is done only when it works and is tested on Windows and Linux; otherwise the gap is an open issue in the parity plan and docs/parity.md shows it. The same behaviour test runs on both CI jobs, only fixtures differ. macOS is a new os/macos/ folder and nothing else. Full text: docs/adr/0006.

## Why

Security promises that silently hold on one OS and not the other are invisible to users and the first thing an auditor finds; a thin OS layer keeps the reviewed code small and makes macOS cheap; one maintainer can't keep three copies in step by hand, but CI can check a rule.

## Alternatives rejected

Ship per-OS as features come and document gaps (the status quo; protections drifted, e.g. FIX-08 Windows-only). A crate per OS (overhead, no thinner interface). Wait for the ADR 0003 service to fix parity (weeks away; PAR-02 and PAR-03 are open today).

## Consequences

Single-OS landings stay open, so features take longer to call done. os/mod.rs is the full list of what KeyHandoff needs from an OS. docs/parity.md replaces scattered OS notes.
