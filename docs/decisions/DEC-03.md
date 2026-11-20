+++
becomes = "history"
date = "2026-11-20"
evidence = ["DOC-01", "SR-12", "SR-13", "SR-14", "SR-15", "SR-16", "base:020da547edb6767be574d345fbfdf7cc4287eaf4"]
id = "DEC-03"
revised_at = "2026-11-20T00:17:16.7709414Z"
scope = [".github/workflows/", "src-tauri/src/update.rs", "docs/adr/0005-a-github-takeover-cannot-ship-an-update.md"]
status = "accepted"
superseded_by = []
supersedes = []
title = "A GitHub takeover can't ship an update"

[author]
agent = "agent-5768"
person = "Spencer Nunamaker"
+++

# DEC-03: A GitHub takeover can't ship an update

## Context

Every installed copy installs updates signed by a key held as a GitHub Actions secret on one maintainer's account, so one account takeover ships code to every user. There is one maintainer and no budget for an HSM or co-signers.

## Decision

Five layers: the account behind two hardware security keys with no SMS or TOTP (SR-12); CI builds but the owner signs offline from an encrypted drive (SR-13); installs wait 48 hours from a signed publish time (SR-14); a spare offline update key can retire a leaked one (SR-15); reproducible builds later (SR-16). Full text: docs/adr/0005.

## Why

Each layer is cheap for one person; together an attacker needs the hardware key, the offline drive and its password, and 48 hours unnoticed, instead of one GitHub login.

## Alternatives rejected

Signing in CI behind required reviewers (the account owner can approve their own deploy); a YubiHSM or cloud KMS (cost, another credential); signing with a YubiKey directly (Tauri's minisign keys can't live on one); turning off auto-update (slower fixes are worse).

## Consequences

Releases need the owner at their machine with the signing drive. Urgent fixes wait 48 hours unless a signed flag says otherwise. Until SR-13 lands, a GitHub takeover can still ship an update.
