# ADR 0004: A backup is the sealed vault with only the recovery-code lock

**Status:** accepted
**Date:** 2026-11-19
**Work:** FEAT-26 (backup and restore), FEAT-27 (check the recovery code at setup)

## Context

A lost or wiped computer loses every kept key, its identity and its contacts. [threats.md](../threats.md#people-and-channels) says "no recovery", and ADR 0003 already promises that "backup is an encrypted vault copy plus the recovery code" without saying what goes in it.

Copying the profile folder by hand works today, but it carries `keyslots.bin` with both ways in. The passphrase slot can be attacked offline: a thief with the file guesses passphrases as fast as Argon2id allows, with no limit on tries. Nothing in a file can count wrong guesses, because whoever holds a copy can reset any counter by taking a fresh copy. Only hardware or a server can rate-limit, and a backup has to travel without either.

Ideas considered on the way here, and why they don't work as a backup:

- **A `.handoff` file sealed to yourself.** It's sealed to a one-time key that lives in the vault and is destroyed when opened, so it needs the very vault it is meant to restore.
- **Something sealed to the TPM.** The TPM is bound to one machine; a backup has to survive losing that machine.
- **An unmodified file (an MP3, say) used as a key.** A file that isn't changed can't hold the secret; the ciphertext still has to live somewhere.

## Decision

A backup is one file holding:

- the sealed profile files (`identity.bin`, `vault.bin`, `contacts.bin`, `teams.bin`, `approvals.bin`), unchanged, still sealed under the master key;
- a `keyslots.bin` with **only the recovery-code slot**.

It never holds the passphrase slot, the TPM slot, or the DPAPI / Secret Service copy kept by Remember on this computer. The last two are device-bound on purpose (SR-06).

**Restore** takes the file and the recovery code, then asks for a new passphrase. It:

- **keeps the identity**, so contacts see the same computer and no continuity warning, and teams still recognise the member;
- **drops every pending one-time key**, so a sealed file whose lock was destroyed after the backup was taken stays unopenable (rule 5). Requests and card locks still outstanding have to be asked for again.

## Why

The recovery code is 160 random bits. At a trillion guesses a second, finding it would take on the order of 10^28 years, so a backup locked only by it is safe to keep on a thumb drive, in cloud storage or anywhere else without hardware or a server. Leaving out the passphrase slot removes the one part of the file that can be guessed. The design reuses slots that already exist, so the vault format doesn't change.

## Alternatives rejected

- **Copy the whole folder.** It carries the guessable passphrase slot.
- **A separate backup passphrase.** It would be weaker than the code unless people choose well, which people don't.
- **A hardware security key slot (FIDO2 PRF).** Portable and rate-limited, but it means buying hardware, against [architecture.md](../architecture.md#key-storage)'s rule. It's filed as an optional extra (FEAT-32), never required.
- **A server that counts guesses.** Against rule 11: no feature depends on a website.

## Consequences

- The recovery code becomes the only way to restore. That makes it worth checking at setup that it was really written down (FEAT-27).
- Losing both the backup's recovery code and the computer still loses the keys. Rotating with the provider stays the fix.
- A backup restored on two computers gives both the same identity. Restore says so, and the old computer should be wiped or reset.
- threats.md and architecture.md change when FEAT-26 lands: "no recovery" becomes "recovery with the backup file and the recovery code".
- If ADR 0003's service moves the vault under a TPM-sealed key, the service still has to keep a recovery-code slot so backups keep working.
