# Design rules

## Purpose

1. Every feature must shorten the time a person or their tools hold a plaintext key.
2. One flow: request, Safe Copy, sealed file, accept, use. API keys are the main case, but any secret works.
3. Encryption is the protection. Sealed files and the vault are ordinary, clearly named files.
4. Prefer designs that eliminate a whole class of problem over checks that need footnotes. A security claim must hold as written; a check that can miss is a warning, never a promise.

## Security

5. A key sent to a person travels sealed to a one-time lock, from a request or a card. Each lock is destroyed when its key is opened, or when it runs out, so a sealed file opens once. A team's keys are sealed to each member's team lock instead, so a later version reaches everyone and leaving the team means being left out of the next version.
6. No way to reveal a key on screen in normal use. Sending leaves nothing new on the sender's computer: you send keys you hold, and the sealed file is deleted when you close it. No servers, accounts or telemetry. KeyHandoff talks to the internet only to check for updates: an installed copy asks GitHub for the latest release, and installs only packages signed with the release key. On the local network it talks to other KeyHandoffs only while Nearby is on. Anything else travels through files and folders the person chooses.
7. Safe Copy listens only when armed, catches one item, times out, and wipes the clipboard and its history.
8. Secret values live only in Rust. The screens get names, states and ids. The one exception is what opens the vault: the passphrase you type and the recovery code shown once at setup pass through the screens and are never stored there.
9. No browser extension. A key copied from a page is on the clipboard for the moment before the wipe, and we say so plainly. Never claim "never on the clipboard" or "you're safe".

## Experience

10. Keys move with copy-and-paste-style actions, not by dragging keys onto windows.
11. Handoffs travel as `.handoff` files whose content is armored text, or through a team folder the team already syncs (Dropbox, Google Drive, OneDrive, a repo, a network share). KeyHandoff never syncs anything itself. No links, and no feature that depends on a website.
12. Accept and Add offer Once, 30 days or Keep, with the last choice preselected.
13. One window, one size. Safe Copy shows inside it and keeps the window's size; only while it's on does the window stay on top. Nothing jumps between screens: what stays the same keeps its place and size.
14. Calm and plain: black ground, white type, one vibrant blue accent, ordinary buttons, quiet motion. Respect `prefers-reduced-motion`.

## Code

15. Tauri with React and TypeScript screens and a Rust core. No Electron.
16. `crates/seal` must not depend on app code.
