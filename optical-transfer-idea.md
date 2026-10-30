# October 30, 2026 — Optical Transfer Idea

> **Status: Exploration only. This is an idea for a future coding agent to consider, not an implementation plan, approved security design, roadmap commitment, or instruction to build.** Treat all security properties as hypotheses requiring threat modeling and empirical tests.

## Concept

**KeyHandoff Lens** would let a user move a one-time secret (for example, a newly generated API key) from an arbitrary desktop website or application into KeyHandoff on a paired phone, using the physical screen as an optical channel. The feature must be **provider-agnostic**: it cannot assume cooperation from Dialpad, AWS, or any other secret issuer. A browser extension should **not** be a prerequisite.

The computer and phone are envisioned as already paired, mutually recognized "friend" devices. The desktop may eventually use a TPM-backed, non-exportable device identity; the phone may use Secure Enclave / Android Keystore-backed keys. Pairing authenticates the receiver and its public keys; it does not make untrusted browser content or arbitrary desktop processes secure.

The core insight: **a recorded visual transfer can be harmless if it contains authenticated ciphertext rather than the secret.** A fullscreen, possibly animated, error-correcting visual code can encode an encrypted payload. It need not resemble a conventional QR code, but visual obscurity is not a security property. A screenshot or screen recording of the transfer should contain only ciphertext.

## Possible user experience (subject to feasibility)

1. On the phone, choose **Receive from computer**. The phone creates a fresh one-time receiving key/request, authenticated within the existing paired-device relationship.
2. On the computer, select/prepare the area where a provider will reveal a secret, and explicitly arm KeyHandoff before the one-time reveal.
3. After the user triggers the provider's Reveal action, a *candidate* native desktop capture mechanism obtains the secret, without intentionally placing it on the clipboard or saving it to disk. This acquisition step is **unresolved**.
4. KeyHandoff encrypts the captured secret for the phone's one-time receiving key, then presents the encrypted result in a fullscreen optical transfer view (for example, a robust animated barcode or custom visual modem).
5. The phone scans and decodes the visual payload locally, authenticates and decrypts it, verifies successful receipt, and stores it in a protected local vault. The desktop closes the transfer view and discards its temporary copies.

The phone camera is outside the computer's screen-capture environment. Its OCR/decoding and vault storage can run locally, without cloud services. An acknowledgment may use an already-paired channel or an optical return path; the precise transport should be evaluated.

## Crucial unresolved security boundary

**A fullscreen privacy shutter does not, by itself, secure plaintext revealed inside an unrelated browser window.** It may hide the underlying page from some *desktop* captures while KeyHandoff uses per-window capture, accessibility APIs, or OCR to access the page. But the same underlying-window content may remain accessible to other capture software, browser internals, local malware, or accessibility clients. An arbitrary opaque overlay is not equivalent to a protected display surface. Also, if the shutter is excluded from recording, the page beneath might become visible to the recorder.

Without provider-side encryption or trusted control of the application's secret-rendering path, **it is not possible to promise that nothing else on the computer can see the revealed secret**. TPM pairing protects device identity and private keys; it does not prevent the provider's plaintext from entering browser memory or capture-accessible surfaces. These limitations must remain explicit in any future design or marketing claims.

## Candidate approaches to evaluate, not commitments

- **Phone-first optical capture:** the user photographs the revealed plaintext directly with the KeyHandoff app; the phone recognizes it locally and stores it securely. Most provider-agnostic and no extension, but does **not** prevent screen recording of the original reveal.
- **Native Safe Copy to optical ciphertext:** reuse the existing Windows KeyHandoff clipboard-capture path, immediately encrypt to the paired phone, and show a fullscreen optical payload. No browser extension and provider-agnostic wherever copying is supported; however, the brief clipboard exposure remains, and clipboard-history cleanup is not a perfect guarantee.
- **Native privacy shutter + underlying-window acquisition:** investigate fullscreen opaque cover, native window capture / accessibility / OCR, encryption, and optical display. It may avoid deliberate clipboard use but must be explicitly treated as experimental until adversarial capture tests establish what it does and does not block.
- **Optional future browser integration:** a generic browser extension might mask a selected DOM element and intercept some reveal workflows earlier. It adds browser compatibility and maintenance costs, will not cover every provider, and cannot guarantee against local malware. It should not be a product prerequisite.

## Existing repo building blocks

- KeyHandoff's `crates/seal` is a standalone Rust protocol library with signed requests, ephemeral/one-time recipient keys, X25519-based sealing and authenticated ciphertext; assess whether its request/identity machinery can be adapted to mobile friend-device pairing.
- KeyHandoff's Rust core intentionally keeps secrets away from React/TypeScript. A prospective optical path should retain that principle.
- KeyHandoff's Windows Safe Copy implementation already listens for a single clipboard event and attempts clipboard/history cleanup. Its documented race and local-malware limitations remain.
- KeyHandoff documents a future hardware-backed TPM/Windows Hello upgrade. That work is not equivalent to protecting plaintext inside third-party browsers.
- MandyGIF (`junovhs/mandygif`) provides region-selection/viewfinder concepts and real Windows Graphics Capture, Linux, and macOS recording backends. It is valuable as an **adversarial recorder**: test both its current WGC capture and historical DXGI Desktop Duplication behavior, plus any separately developed capture techniques, against a synthetic reveal. Do not assume any Netflix/Hulu protected-video bypass is already proven in the checked-in implementation.

## Suggested research questions for a future coding agent

1. Can a fully provider-agnostic, no-extension native workflow acquire the original secret without clipboard use and without exposing it to independent window capture? Test, do not assume.
2. What does a fullscreen shutter hide from Snipping Tool, OBS, Windows Graphics Capture, DXGI duplication, browser/window capture, accessibility and MandyGIF? Are there exposure frames during arming, reveal or teardown?
3. Can existing `crates/seal` requests and wire formats support paired mobile devices and authenticated, one-time optical receipts without weakening replay or identity checks?
4. What is the smallest reliable optical transport for typical API keys (payload size, static vs animated frames, frame timing, error correction, camera glare, focus, checksum and confirmation)?
5. What can iOS/Android secure-storage and on-device camera-processing APIs actually guarantee about captured frames, memory, screenshots, and temporary files?
6. What security promise remains defensible without provider cooperation? Separate **ciphertext-only transfer visibility** from **plaintext protection during acquisition**.

**No code changes are requested by this note.** A future agent should first produce a threat model and small synthetic-secret experiments, then recommend whether any version of this idea warrants a real implementation plan.
