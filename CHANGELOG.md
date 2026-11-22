# Changelog

Each version's notes become its GitHub Release. To release, set `version` in
`src-tauri/Cargo.toml` and add its section here; the release workflow builds and
publishes it once both are on `main`.

## [0.3.0] - 2026-11-22

KeyHandoff is still pre-release and has not had a security audit: use it with
test keys.

- **KeyHandoff for macOS:** one app for Intel and Apple silicon Macs, macOS
  10.15 or later. The vault, cards and sealed keys, teams and team folders,
  Nearby, Safe Copy, `keyhandoff run` and the loaders all work as on Windows
  and Linux. Double-click a `.handoff` file in Finder to open it.
- **Safe Copy on the Mac:** it checks the clipboard only while it listens,
  takes your one copy, clears it, and names clipboard managers such as Maccy,
  Paste, Alfred or Raycast if they're running.
- **Programs and AI agents on the Mac:** KeyHandoff names the program asking
  for a key from macOS itself, recognises Claude Code and the other agents,
  and always asks when a cron job or a launchd agent is the one asking.
  Node.js programs get stand-in keys; other programs get the key itself for
  now, with every printed copy hidden.
- **Type a key into a field on the Mac:** allow KeyHandoff under Privacy &
  Security, Accessibility once, and Type works as on Linux.
- **Remember on this computer** keeps its copy in the Mac's login Keychain.
- **The `keyhandoff` command** is linked into `/usr/local/bin` when KeyHandoff
  can write there; the install guide has the one-line command otherwise.
- **Safe Copy shows what it checked** while it listens, instead of staying
  quiet when all is well.
- **Sealed keys show as a card** in Send a key: drag it into a chat, an email
  or a folder, or copy it as text.

The Mac app isn't notarized yet: the first time, right-click it in
Applications and choose Open (on macOS 15, System Settings, Privacy &
Security, Open Anyway).

## [0.2.0] - 2026-11-20

KeyHandoff is still pre-release and has not had a security audit: use it with
test keys.

- **A new look:** the vault is a one-line list of your keys, built around
  finding, using and handing one off, in slick black with a pearl accent.
- **Import a .env:** KeyHandoff seals the file's keys into the vault and leaves
  `handoff:` pointers behind, so the plain keys are gone from the file.
- **`keyhandoff run -- COMMAND`:** starts any program with the keys its `.env`
  points to, with no code changes. Works from Windows terminals too; the
  installer puts the `keyhandoff` command on your PATH.
- **Stand-in keys:** under `keyhandoff run`, a program gets a look-alike key
  instead of the real one. The real key is swapped in only on the way to that
  key's own server. If the stand-in goes anywhere else, the send is stopped
  before a byte leaves and you get an alert. On Linux this covers every
  program; on Windows, Node.js programs for now.
- **Teams in a shared folder:** keep a team in a folder its members already
  sync. Rotating a key reaches everyone, a keeper can take someone off, and a
  place one person's program was stopped sending to is refused for the whole
  team.
- **Nearby:** add someone in the same room and send keys straight to contacts
  on your local network. Off until you turn it on.
- **AI coding agents:** KeyHandoff recognises Claude Code and Codex. Saved
  approvals don't count for them; you can allow one agent session instead, see
  what it ran, and revoke it.
- **Scheduled jobs and services** (Task Scheduler, Windows services, cron,
  systemd) always ask; they never reuse an approval.
- **Backups:** Save a backup writes a `.khbackup` file that opens only with
  your recovery code. After setup, you type back two groups of the recovery
  code to show you wrote it down.
- **Safer updates:** releases are signed offline, never by GitHub, and
  installed copies wait 48 hours after a release was signed before taking it.
- **Fixes:** many Windows pipe and process fixes, and a resize glitch on Linux.
- **Open source** under Apache 2.0.

## [0.1.0] - 2026-10-31

The first build to install. KeyHandoff is pre-release and has not had a security
audit: use it with test keys.

- **Installers:** a Windows installer, and a Linux `.deb` and AppImage (X11).
  Opening a `.handoff` file starts KeyHandoff.
- **Updates:** installed copies check for a new version at launch and every six
  hours, and install only updates signed with the release key. While
  KeyHandoff is locked they install and restart on their own; otherwise
  **Restart to update** shows in the title bar.
- **Cards and sending:** share your card once; send a teammate any key you hold
  as a sealed file that opens once, on their computer. Teams get one file for
  everyone.
- **Your keys:** Add a key with Safe Copy; keep it once, 30 days or for good.
- **Keys in code:** `.env` pointers such as `DIALPAD_API_KEY=handoff:Dialpad`,
  loaded by the Python and Node loaders after you allow the program.
- **Locking:** a passphrase or a TPM PIN, a printed recovery code, and locking
  on sleep, screen lock or after a quiet spell you choose.
