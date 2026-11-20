# ADR 0005: A GitHub takeover can't ship an update

**Status:** accepted
**Date:** 2026-11-19
**Work:** SR-12 (owner's account), SR-13 (offline signing), SR-14 (install delay), SR-15 (spare key), SR-16 (reproducible builds)

## Context

Every installed copy installs any update signed with the release key ([threats.md](../threats.md#on-the-receivers-computer)). That key is a GitHub Actions secret on one maintainer's account. So taking over that GitHub account, by phishing a password or a one-time code, is enough to put code on every computer running KeyHandoff. For a business holding customers' payment details, that outweighs any detail of the cryptography.

KeyHandoff has one maintainer. The answer can't be "get a second signer" or "buy an HSM"; it has to work for one person at small scale.

## Decision

Make each step an attacker would need a separate thing they can't get from GitHub:

1. **The account is behind hardware security keys** (SR-12). Two YubiKeys (a primary and a spare kept elsewhere), no SMS or authenticator-app fallback, and recovery codes on paper. Phishing can't relay a hardware key's answer to another site.
2. **Releases are signed offline** (SR-13). CI builds and drafts the release but holds no signing key. The owner downloads the packages, checks their hashes against the CI log, signs them on their own machine with a key kept on an encrypted drive used only for this, and uploads the signatures. A GitHub takeover can publish a release, but nothing will install it.
3. **Installed copies wait 48 hours** (SR-14). The wait is measured from a publish time inside the signed manifest. GitHub emails the owner on every release, so a release they didn't make can be pulled before anyone installs it.
4. **A spare update key** (SR-15). The app trusts a second public key whose private half is made offline and never used. An update signed by it replaces a leaked working key.
5. **Reproducible builds, later** (SR-16). Anyone can rebuild a tag and check it matches the release, so trust doesn't rest on one person's machine.

## Why

Each layer is cheap for one person. Together, an attacker needs the hardware key, the offline signing drive and its password, and to go unnoticed for 48 hours. Today they need one GitHub login. The order follows how likely each attack is: phishing the account first, then a key stolen from CI, then a key that leaks some other way.

## Alternatives rejected

- **Keep signing in CI with GitHub Environments and required reviewers.** Whoever controls the account can approve their own deployment.
- **A YubiHSM or cloud KMS for the signing key.** Costly, and the KMS credential is one more secret to steal; the encrypted drive gives most of the benefit.
- **Sign with a YubiKey directly.** Tauri's updater uses minisign keys, which a YubiKey can't hold; it can be revisited if the updater moves to keys a YubiKey can hold.
- **No auto-update.** Slow security fixes are worse for users than a well-guarded update key.

## Consequences

- Publishing a release needs the owner at their own computer with the signing drive. Releases take minutes longer.
- An urgent fix still waits 48 hours unless its signed manifest marks it urgent. That flag is itself signed offline, so an attacker can't set it.
- Losing the signing drive without the spare means users have to reinstall by hand once. The spare key exists to avoid that.
- threats.md's update section changes as each issue lands. Until SR-13 lands, it stays true that a GitHub takeover can ship an update.
