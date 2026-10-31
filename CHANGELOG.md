# Changelog

Each version's notes become its GitHub Release. To release, set `version` in
`src-tauri/Cargo.toml` and add its section here; the release workflow builds and
publishes it once both are on `main`.

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
