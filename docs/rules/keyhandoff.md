# Design rules

## Purpose

1. Every feature must shorten the time a person or their tools hold a plaintext key.
2. One flow: request, Safe Copy, sealed file, accept, use. API keys are the main case, but any secret works.
3. Encryption is the protection. Sealed files and the vault are ordinary, clearly named files.
4. Prefer designs that eliminate a whole class of problem over checks that need footnotes. A security claim must hold as written; a check that can miss is a warning, never a promise.

## Security

5. Every handoff is a request then a sealed reply. Each request carries a fresh one-time key, destroyed on accept or expiry.
6. No way to reveal a key on screen in normal use. The sender keeps nothing. No servers, accounts or telemetry.
7. Safe Copy listens only when armed, catches one item, times out, and wipes the clipboard and its history.
8. Secret values live only in Rust. The screens get names, states and ids.
9. No browser extension. A key copied from a page is on the clipboard for the moment before the wipe, and we say so plainly. Never claim "never on the clipboard" or "you're safe".

## Experience

10. Keys move with copy-and-paste-style actions, not by dragging keys onto windows.
11. Handoffs travel as `.handoff` files whose content is armored text. No links, and no feature that depends on a website.
12. Accept and Add offer Once, 30 days or Keep, with the last choice preselected.
13. One window. Safe Copy shows inside it, and only while it's on does the window go compact and stay on top.
14. Calm and plain: black ground, white type, one vibrant blue accent, ordinary buttons, quiet motion. Respect `prefers-reduced-motion`.

## Code

15. Tauri with React and TypeScript screens and a Rust core. No Electron.
16. `crates/seal` must not depend on app code.
