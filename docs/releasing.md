# Releasing

Installed copies install any update signed with the updater key, so that key decides what runs on every user's computer. It lives only on the owner's signing drive. CI builds and drafts each release but can't sign it, so taking over the GitHub account can't ship an update ([ADR 0005](adr/0005-a-github-takeover-cannot-ship-an-update.md)).

## A release, step by step

1. **Bump and describe.** Set `version` in `src-tauri/Cargo.toml` and add its section to `CHANGELOG.md`, then land both on `main`.
2. **CI drafts it.** The `release` workflow builds the Windows installer, the `.deb` and the AppImage, and creates a **draft** release with them and `SHA256SUMS`. A draft has no tag. Installed copies and the website can't see it. The `draft` step's log prints the hashes.
3. **Sign it on your machine.** Plug in and unlock the signing drive, then from a checkout of `main`:

   ```bash
   TAURI_SIGNING_PRIVATE_KEY_PATH=/path/to/drive/keyhandoff.key scripts/sign-release.sh 0.2.0
   ```

   The script:
   - checks the draft targets a commit on `main` whose version is the one you named;
   - downloads the packages and checks them against `SHA256SUMS`;
   - prints the hashes so you can compare them with the `draft` step's log;
   - asks for the key's password once, without showing it;
   - signs each package;
   - checks every signature against the public key in `src-tauri/tauri.conf.json`, so a wrong key file is caught before anything is uploaded;
   - uploads the signatures and `latest.json`.
4. **Publish.** The script asks `Publish v0.2.0 to everyone now?`. Answering `y` publishes the release. That creates the tag, the script deploys the website from your computer so its download links follow (`scripts/deploy-site.sh`), and installed copies take it **48 hours after you signed it** (SR-14). That window is for noticing a release you didn't make: GitHub emails you about every release, so pull any you don't recognise. There's no way to skip the wait yet, so even an urgent fix takes two days to reach installed copies; new downloads from the website get it at once. Any other answer leaves a signed draft; publish it later with `gh release edit v0.2.0 --draft=false --latest`.
5. **Unplug the signing drive.**

The script needs `gh` (signed in), `git`, `jq`, `node`, `npx` and `sha256sum`. Git Bash on Windows has all of them once `gh` and Node are installed.

## The signing drive

- An encrypted USB drive (BitLocker To Go or VeraCrypt) used for nothing else. It holds the updater private key (`keyhandoff.key`) and nothing secret besides.
- The key's password is a passphrase that isn't stored anywhere on a computer.
- Keep a second copy of the drive somewhere else, such as a safe deposit box or a fireproof safe at home. Losing every copy means users must reinstall by hand to get the next update, until SR-15 adds a spare key.
- Plug it in only to sign.

## One-time switch from CI signing

Until SR-13, CI signed releases with the `TAURI_SIGNING_PRIVATE_KEY` repository secret. GitHub never shows a secret again, so the switch depends on whether you still have the key file that secret came from.

**If you have the original key file** (for example `~/.tauri/keyhandoff.key` on the machine that made it, or a copy in a password manager):

1. Copy it to the signing drive and delete every other copy.
2. Delete the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets under **Settings → Secrets and variables → Actions**.

Nothing changes for installed copies.

**If the secret is the only copy:**

1. On the signing drive, make a new key: `npx tauri signer generate -w /path/to/drive/keyhandoff.key`, with a new passphrase.
2. Put the new public key (the `.pub` file's contents) in `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`, and land it with the next version bump.
3. Delete both secrets.

Copies already installed trust the old key, so they won't accept updates signed with the new one. Their users install the next release by hand once, from the website, and updates work again from then on. Only v0.1.0, a pre-release, is affected.

Either way, delete the secrets. While they exist, anyone who controls the account could still sign with them.

## What this does and doesn't stop

- **A GitHub takeover** can change code, run CI and publish releases, but it can't sign them. Installed copies refuse an unsigned or wrongly signed package.
- **A package swapped on the draft after CI built it** no longer matches the hashes in the `draft` step's log. The script's own `SHA256SUMS` check only catches damage in transit, since whoever swaps a package can swap that file too, so compare with the log. Sign only drafts you started with a version bump.
- **A build CI itself was made to produce wrongly** (a changed workflow, say) has matching hashes everywhere. Read the commits since the last release before signing. Reproducible builds (SR-16) will let anyone check a package against the source.
- **A stolen signing drive** is still protected by the key's passphrase.
- **Your own computer, if compromised while the drive is plugged in,** could sign something else. Keep the drive in only for the minute it takes.

## The website

keyhandoff.dev is deployed from the owner's computer with `scripts/deploy-site.sh`, never from GitHub Actions, so the repository holds no Cloudflare credential and taking over the GitHub account can't change the site's download links. The `site` workflow only checks the site builds. Run the script from an up-to-date `main` after site changes land; `sign-release.sh` runs it after publishing. It uses your own `npx wrangler login` and waits until the live site serves the commit it built.
