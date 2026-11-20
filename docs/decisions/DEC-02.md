+++
becomes = "history"
date = "2026-11-20"
evidence = ["DOC-01", "FEAT-26", "FEAT-27", "base:020da547edb6767be574d345fbfdf7cc4287eaf4"]
id = "DEC-02"
revised_at = "2026-11-20T00:17:11.7125571Z"
scope = ["src-tauri/src/slots.rs", "src-tauri/src/store.rs", "docs/adr/0004-backup-is-the-vault-with-only-the-recovery-code.md"]
status = "accepted"
superseded_by = []
supersedes = []
title = "A backup is the sealed vault with only the recovery-code lock"

[author]
agent = "agent-5768"
person = "Spencer Nunamaker"
+++

# DEC-02: A backup is the sealed vault with only the recovery-code lock

## Context

A lost computer loses every key. Copying the profile folder carries the passphrase slot, which a thief can guess offline with no limit; no file can count wrong guesses, since a copy resets any counter.

## Decision

A backup holds the sealed profile files and a keyslots copy with only the recovery-code slot: no passphrase, TPM or Remember copy. Restore needs the recovery code, sets a new passphrase, keeps the identity and drops pending one-time keys. Full text: docs/adr/0004.

## Why

The recovery code is 160 random bits and can't be guessed; leaving out the passphrase slot removes the only guessable part, and it reuses existing slots without a format change.

## Alternatives rejected

Copy the whole folder (guessable passphrase slot); a separate backup passphrase (weaker in practice); a FIDO2 hardware-key slot (needs buying hardware, filed as optional FEAT-32); a guess-counting server (no features depend on a website); a .handoff file sealed to yourself or a TPM-sealed copy (both need the lost machine).

## Consequences

The recovery code becomes the only way to restore, so setup should check it (FEAT-27). threats.md's 'no recovery' changes when FEAT-26 lands. A service-held vault (ADR 0003) must keep a recovery-code slot.
