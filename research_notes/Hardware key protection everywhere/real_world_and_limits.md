# Real-world vault key protection, same-user attacker limits, and the best software fallback

Research date: 2026-10-31. Primary sources were read where possible (Bitwarden and KeePassXC source code on GitHub via `gh api`, Microsoft Learn, kernel.org, Apple Platform Security, OWASP, RFC 9106, Elastic Security Labs). Secondary and forum sources are labeled as such. "Documented" means the vendor or code says so; "claim" means a vendor assertion or third-party report that was not independently checked.

## 1. How leading products wrap the vault key on each OS, what the prompt is, and whether biometric unlock is a cryptographic gate or a convenience cache

### Takeaway
Leading products all use a two-tier pattern. (a) A strong secret (a master password, plus sometimes a 128-bit Secret Key) is the real root. (b) A "quick unlock" tier rests on the OS. On Windows the best designs (Bitwarden "sign path", KeePassXC) are a real cryptographic gate: the vault key is wrapped with a hash of a Windows Hello signature over a stored random challenge, and the signing key lives in the TPM-backed Hello store. On Linux, polkit gives only yes/no, so every product either keeps the key in memory only (lost at restart) or admits that a persistent key in the Secret Service is readable by any same-user process. Plain DPAPI or Secret Service storage (Signal after 2024, Chrome before ABE, gh, git-credential-manager, aws-vault defaults) protects only against offline disk theft, never against same-user code.

### Cited Findings

**Bitwarden desktop (Rust `desktop_native/biometric`, read from main branch, Oct 2026)**
- The stated security goal: "a locked vault - a running app - cannot be unlocked when the device (user-space) is compromised in this state." — [bitwarden/clients biometric/src/windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs)
- Windows has two paths. The **UV path** (used after the first unlock in a running app) keeps the user key in process memory, encrypted under DPAPI (`CryptProtectMemory`), and asks Windows Hello for a yes/no ("Get a yes/no authorization without any cryptographic backing"). The code itself notes "Further process isolation is needed here so that code cannot be injected into the running process, which may circumvent DPAPI." — [windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs)
- The **Sign path** (persistent, works after restart) signs a per-user random challenge with the Windows Hello key store. The result is "a deterministic signature which is hashed to obtain a key." That key seals the user key into a `SecretProtectedKeyEnvelope`, and the challenge plus envelope go into the keychain. Each enrollment gets a unique challenge. — [windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs)
- Bitwarden's own statement of the remaining attack: "Since the keychain can be accessed by all user-space processes, the challenge is known to all userspace processes. Therefore, to circumvent the security measure, the attacker would need to create a fake Windows-Hello prompt, and get the user to confirm it." — [windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs)
- The prompt has to be in the foreground for face or fingerprint to work, so the code grabs the foreground window (desktop app or browser) to parent it. — [windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs)
- Linux uses polkit. **Ephemeral path:** the user key is held in memory protected by `memfd_secret`, and polkit gives a yes/no before the key is released. **Persistent path (opt-in):** "Unlike Windows Hello, polkit only returns a yes/no authorization decision and no key material, so there is no secret to wrap the user key with; the Secret Service is the only thing protecting it at rest... any un-sandboxed process running as the user can read the key from the Secret Service and thus unlock a locked vault without user verification. Because of this it is opt-in only, gated behind an explicit warning." — [bitwarden/clients biometric/src/linux.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/linux.rs)
- The polkit check "relies on no custom rules in the system skipping the authorization check." — [linux.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/linux.rs)
- The `secure_memory` crate keeps secrets AES-256-GCM-encrypted in process memory under a per-process key. That key is protected by DPAPI `CryptProtectMemory` on Windows, by the kernel keyring (`keyctl`) or `memfd_secret` on Linux, and by an `mlock`ed allocation as fallback. It is "defense-in-depth for a *running, locked* app... does not defend against a compromised kernel or administrator." — [secure_memory/README.md](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/secure_memory/README.md)

**KeePassXC (develop branch source)**
- Windows Hello quick unlock: a random challenge (also used as the AES-GCM IV) is signed by a Windows Hello `KeyCredential` (`RequestSignAsync`). "Use the SHA-256 hash of the challenge signature as the encryption key," and the database credentials are encrypted with AES-256-GCM. — [keepassxc src/quickunlock/WindowsHello.cpp](https://github.com/keepassxreboot/keepassxc/blob/develop/src/quickunlock/WindowsHello.cpp)
- The original MVP (PR #7384, merged Feb 2022) described the Hello key store as "typically TPM-backed." — [keepassxc PR #7384](https://github.com/keepassxreboot/keepassxc/pull/7384)
- Linux Polkit quick unlock (2023 code) makes a random AES-256-GCM key and IV and stores them in the **kernel process keyring** (`add_key("user", ..., KEY_SPEC_PROCESS_KEYRING)`). It encrypts the master key with them, keeps the ciphertext in memory (`m_encryptedMasterKeys`), and asks polkit with AllowUserInteraction. The design is ephemeral: it lives in the process and is gone when the process ends. — [keepassxc src/quickunlock/Polkit.cpp](https://github.com/keepassxreboot/keepassxc/blob/develop/src/quickunlock/Polkit.cpp)
- macOS Touch ID is in `TouchID.mm` (not read in detail). — [keepassxc tree](https://github.com/keepassxreboot/keepassxc/tree/develop/src/quickunlock)

**1Password**
- Windows Hello without TPM mode: the account password is needed again after 1Password restarts (update or reboot), after the "Confirm my account password" interval, on biometric failure, or after a password change. With "Use your device's Trusted Platform Module to unlock" turned on, Hello can unlock after quit or restart. — [1Password Support: Windows Hello](https://support.1password.com/windows-hello/)
- A 1Password staff explanation (forum, secondary): "the only way we can safely persist the encryption keys ... across device reboots or after quitting 1Password is to store the encryption keys in a TPM." With TPM mode on, "1Password loses control over what can prompt you to access the key 1Password creates on the TPM" because "1Password delegates the responsibility of authentication to Windows Hello." — [1Password Community thread](https://www.1password.community/1password-at-home-31/security-with-use-the-trusted-platform-module-with-windows-hello-19040)
- Staff also said Unlock with SSO on Windows is higher risk "since the key protection mechanisms we use on macOS, iOS and Android aren't available here." — [1Password Community](https://1password.community/discussion/comment/675700)
- Linux system authentication: the password is required again after 1Password restarts (update or reboot), after the confirm interval, and on failure. The key holding mechanism is not described on the user page. — [1Password Support: Linux system authentication](https://support.1password.com/system-authentication-linux/)
- The Secret Key is 34 characters with 128 bits of entropy, generated on the device, never sent to 1Password, and kept in the Emergency Kit and on signed-in devices. It protects against server-side brute force, and "We have no record of your Secret Key and can't recover it." Account passwords "average about 40 bits." — [1Password Support: About your Secret Key](https://support.1password.com/secret-key-security/)
- Two-Secret Key Derivation combines password and Secret Key into both an auth secret and a key-encryption key (BSidesLV 2016 talk). A third party reports PBKDF2-SHA256 at 650,000 iterations plus HKDF and SRP, but this was not confirmed against the official whitepaper. — [BSidesLV 2016 talk listing](https://infocondb.org/con/security-bsides/bsideslv-2016/what-weve-learned-with-two-secret-key-derivation)

**Apple Keychain / Passwords (the macOS target)**
- securityd checks keychain-access-groups, application-identifier and app-group entitlements, so items are per app or developer, not per user. "The secret key always requires a round trip through the Secure Enclave." ACLs are "evaluated inside the Secure Enclave and are released to the kernel only if their specified constraints are met." Items can require user presence (Touch ID, Face ID, passcode), and can require that biometric enrollment hasn't changed since the item was added. — [Apple Platform Security: Keychain data protection](https://support.apple.com/guide/security/keychain-data-protection-secb0694df1a/web)

**Signal Desktop**
- In July 2024 Signal moved its SQLCipher key from plaintext `config.json` to Electron `safeStorage`. On macOS that means the Keychain (per app). On Windows and Linux access is per user, so same-user malware "can decrypt the database encryption key through safeStorage." The author (SandboxAQ, Nov 14 2024) prototyped a YubiKey 5 (firmware 5.7+ PIV X25519) design with touch or PIN. They rejected the TPM because X25519 isn't mandatory or implemented in PC TPMs. — [SandboxAQ: Protecting Signal Keys on Desktop](https://cryptographycaffe.sandboxaq.com/posts/protecting-signal-desktop-keys/)
- The rollout kept a plaintext fallback for a time. Linux users hit "can't decrypt DB key (previous: kwallet5, current: kwallet6)" when the keyring backend changed, and recovery meant relinking and losing history. — [Gentoo Forums, Sept 2024](https://forums.gentoo.org/viewtopic-p-8841636.html); [yingtongli.me Aug 2025 migration guide](https://www.yingtongli.me/blog/2025/08/13/signal-secrets.html)

**Chrome App-Bound Encryption (ABE), Windows, Chrome 127, July 2024**
- Google announced it July 30, 2024 ("Improving the security of Chrome cookies on Windows"). The body could not be fetched. — [Google Security Blog](https://security.googleblog.com/2024/07/improving-security-of-chrome-cookies-on.html)
- Mechanism (per Elastic): keys sit in encrypted files, and a SYSTEM-level elevation service releases a key only after confirming the request comes from the Chrome process. Elastic calls it "not a panacea." — [Elastic Security Labs, "Katz and Mouse Game", Oct 28 2024](https://www.elastic.co/security-labs/katz-and-mouse-game)
- It is on by default in Chrome and Edge after v127. — [Red Canary](https://redcanary.com/blog/threat-intelligence/google-chrome-app-bound-encryption/)
- Bypass timeline: see section 2.

**Proton Pass, Dashlane**
- Proton Pass desktop biometric unlock uses Windows Hello and Touch ID and is a paid-tier feature. No public technical description of local key storage was found. — [Proton support](https://proton.me/support/biometric-unlock-desktop); [Engadget](https://engadget.com/cybersecurity/proton-pass-now-supports-touch-id-and-windows-hello-biometric-logins-100025225.html)

### Inferences
- **The Windows pattern to copy is Bitwarden "sign path" / KeePassXC:** make a Windows Hello `KeyCredential` (via `KeyCredentialManager`), sign a stored random challenge, hash or HKDF the signature, and AEAD-wrap the vault key. This is a cryptographic gate: without a Hello signature the wrapped blob cannot be opened. It depends on the Hello signature being deterministic for a given challenge, which both projects rely on. Hello keys are RSA with PKCS#1 v1.5-style deterministic signatures; that last point is from background knowledge and should be checked.
- **The gate is "a Windows Hello prompt the user approves," not "only KeyHandoff."** Any same-user process can open the same named KeyCredential and call `RequestSignAsync`, which shows a real Hello prompt. 1Password's "loses control over what can prompt" and Bitwarden's "attacker would need to ... get the user to confirm it" both say this. So docs should say: malware can trigger a genuine Hello prompt, and a user who approves it unexpectedly gives the key away.
- **On Linux, polkit gives no cryptographic gate.** Bitwarden, KeePassXC and 1Password all keep a polkit-gated key in memory only, so the user re-enters the password after restart. Persisting via the Secret Service reduces to today's KeyHandoff behavior. A cryptographic gate on Linux needs a TPM2 (PIN policy with dictionary-attack lockout) or a FIDO2 key (hmac-secret) — see sections 5 and 6.
- **DPAPI or Secret Service wrapping alone (KeyHandoff today) is the same tier as Signal post-2024 and pre-ABE Chrome:** it stops disk theft and backup leaks, not same-user code.

### Gaps
- 1Password's exact Windows TPM-mode mechanism (which API, key type, PCR or PIN policy) is undocumented. The support security sub-page and white paper were not retrieved.
- Dashlane desktop local key storage: no source found. Dashlane has deprecated its desktop app in favor of the extension; not verified this session.
- Proton Pass desktop key wrapping: no technical source found.
- KeePassXC Touch ID details (Secure Enclave key vs keychain item with biometry ACL) were not read.
- Bitwarden's macOS path is not in the Rust biometric crate (`unimplemented.rs`). It appears to be in TypeScript or Swift (`os-biometrics-mac.service.ts`) and was not read.

## 2. Infostealer reality: how stealers take DPAPI-protected secrets and password-manager data, whether prompt-gated hardware keys have stopped them, and what they do instead

### Takeaway
Commodity stealers run as the user and treat DPAPI-protected secrets as plaintext. When Chrome added a caller-identity check (ABE), five or more families bypassed it within roughly two to three months. None of them broke the crypto. They injected into Chrome, read its memory, drove it over the debug port, impersonated SYSTEM, or set the policy registry key to turn ABE off. Stealers increasingly target password managers: their files, their memory while unlocked, and the moment of unlock. No source found shows a stealer defeating a prompt-gated hardware key cryptographically. The documented responses are to wait for the vault to be unlocked and read memory, or to capture the master password.

### Cited Findings
- **ABE bypasses, with dates (Elastic, Oct 28 2024):**
  - STEALC/VIDAR (about Sept 20, 2024) restarted Chrome on a hidden desktop and read unencrypted cookies from the network-service process memory (ChromeKatz technique).
  - METASTEALER (Sept 30) impersonated the SYSTEM token and called the elevation service's COM `DecryptData`. Elastic notes this "requires elevated privileges despite marketing claims."
  - PHEMEDRONE (late Sept) used an off-screen Chrome with `--remote-debugging-port=9222` and `Network.getAllCookies`.
  - XENOSTEALER (Sept 26) injected into chrome.exe with SharpInjector and called `DecryptData` from inside.
  - LUMMA (mid-Oct) copied `chrome.dll` from memory and pattern-scanned for CookieMonster, confirmed on Chrome 130.0.6723.70.
  - The first bypasses came about two months after the July 2024 release. — [Elastic Security Labs](https://www.elastic.co/security-labs/katz-and-mouse-game)
- Vidar was also seen using APC injection targeting `CryptUnprotectMemory` inside the browser. Some Stealc, Vidar and Lumma versions use remote debugging, and Lumma has spawned its own Chrome child to dump memory. — [Red Canary](https://redcanary.com/blog/threat-intelligence/google-chrome-app-bound-encryption/); [CyberInsider summarizing Elastic](https://cyberinsider.com/this-is-how-infostealers-bypass-chromes-latest-cookie-security/)
- Adversaries also set the `ApplicationBoundEncryptionEnabled` policy registry value to 0, seen in VirusTotal samples. — [Zscaler, Nov 13 2024](https://www.zscaler.com/blogs/cybersecurity-best-practices/google-chrome-app-bound-encryption)
- Elastic's detection ideas: cookie-file access by rare processes, Chrome launched by unusual parents, connections to port 9222, unsigned DLLs in the Chrome folder (which the elevation service trusts), and access to the elevation-service CLSID. — [Elastic](https://www.elastic.co/security-labs/katz-and-mouse-game)
- **Password managers as targets:**
  - Check Point (2026) describes the "Aokigahara" stealer, whose `passman` module targets KeePass, Bitwarden, Bitwarden CLI, 1Password, Dashlane, RoboForm, NordPass and Sticky Password data and config files. — [Check Point Research, 2026](https://research.checkpoint.com/2026/infostealer-riding-the-ai-train/)
  - Jamf Threat Labs (reported July 2026, secondary) found CrashStealer (macOS) code targeting 14 password managers including 1Password, Bitwarden, KeePassXC and Keeper. It notes a growing focus on OS keychains, process memory, sessions, the clipboard, and "the moment a user unlocks a password manager." — [Lunar Cyber summary](https://lunarcyber.com/blog/password-manager-stealers-how-malware-targets-1password-bitwarden-lastpass-dashlane-and-browser-vaults/)
  - Rhadamanthys is reported to extract KeePass credentials. The article also explains reading the RAM of password-manager processes (date unclear, secondary). — [Twilight Cyber](https://twilightcyber.com/password-managers-vs-infostealers/)
- Vendor positions:
  - Bitwarden community: on Windows "any user-space software can read the memory of another," so malware can dump an unlocked Bitwarden. — [Bitwarden Community Forums](https://community.bitwarden.com/t/do-password-managers-really-not-protect-from-info-password-stealers/92152)
  - KeePass's maintainer: no password manager can run securely "in a spyware-infected, insecure environment" (paraphrase via search summary). — same thread / [KeePass Wikipedia](https://en.wikipedia.org/wiki/KeePass)

### Inferences
- **Lesson from ABE for KeyHandoff:** a broker that checks "is the caller the right exe or path" loses to same-user injection into, or relaunch of, the trusted process within weeks. A user-presence prompt tied to a hardware key changes the attacker's cost from "run code" to "get the user to approve, or wait for an unlock." That is a real improvement, and it should be described honestly as such.
- Expected attacker behavior against a prompt-gated KeyHandoff: steal the encrypted vault file and wrapped-key blob (cheap; useless without the TPM or Hello or a cracked passphrase), then wait for the app to unlock and read memory, keylog the passphrase in fallback mode, or scrape the clipboard (which HAND-39's tripwire already partly addresses), or trigger a genuine Hello prompt at a believable moment.
- Signed, per-use confirmation for "copy or send key" actions moves the target from "the key while unlocked" to "each use," which narrows what a memory reader gets only if keys are not kept decrypted in memory between uses.

### Gaps
- No public report was found measuring stealer success against hardware-gated password manager quick-unlock specifically (for example, Windows Hello sign-path Bitwarden). Absence of evidence is not evidence of resistance.
- Mandiant, SpyCloud and Recorded Future reports were not retrieved this session. Their numbers (infection counts, share of stealer logs with password-manager data) are therefore missing.
- Whether stealers in 2025–2026 bypass Chrome's later ABE hardening (for example, ABE for passwords and IBANs, or Chrome's 2025 changes to remote debugging on the default profile) was not checked.

## 3. What same-user malware can still do while the vault is unlocked, and practical mitigations

### Takeaway
Once the key is in a normal-user process, same-user code can usually read it. The paths are OpenProcess/ReadProcessMemory on Windows (the default process DACL comes from the user's token), ptrace or `/proc/pid/mem` on Linux unless Yama or `PR_SET_DUMPABLE` blocks it, injection, the debug port, screen capture, input injection, and prompt spoofing. Protected processes block VM_READ, but normal apps can't use them. Mitigations raise cost but don't make a boundary: restrictive process DACL (Windows), `prctl(PR_SET_DUMPABLE,0)` plus `RLIMIT_CORE=0` (Linux), encrypted-in-memory keys (`memfd_secret`, keyctl, `CryptProtectMemory`), short unlock windows, and per-use confirmation for high-value actions.

### Cited Findings
- **Windows access model:**
  - `OpenProcess` checks the requested rights against the target's DACL. `PROCESS_VM_READ` is required for `ReadProcessMemory`, and `PROCESS_VM_WRITE`/`VM_OPERATION`/`CREATE_THREAD` are needed for writes and injection.
  - The default process DACL comes "from the primary or impersonation token of the creator," which is why same-user processes can normally read each other.
  - `SeDebugPrivilege` grants full access. A handle with `PROCESS_DUP_HANDLE` can be escalated to maximum access.
  - Protected processes deny READ_CONTROL, WRITE_DAC, WRITE_OWNER, VM_READ/WRITE/OPERATION and CREATE_THREAD from normal processes.
  - `SetSecurityInfo` changes a process's security descriptor; WRITE_DAC is "required to modify the DACL." — [Microsoft Learn: Process Security and Access Rights](https://learn.microsoft.com/en-us/windows/win32/procthread/process-security-and-access-rights)
- **Bitwarden's process isolation:**
  - Windows calls `secmem_proc::harden_process()` to set a restrictive DACL. Linux sets `prctl(PR_SET_DUMPABLE, 0)` ("no other running process (root or same user) can dump the memory of this process or attach a debugger") and `RLIMIT_CORE=0`. — [bitwarden/clients core/src/process_isolation/linux.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/core/src/process_isolation/linux.rs); [windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/core/src/process_isolation/windows.rs)
  - The Bitwarden comment's "root" claim is too strong: root with CAP_SYS_PTRACE can still attach to a non-dumpable process. This is from background knowledge; `PR_SET_DUMPABLE=0` mainly blocks same-UID attach and `/proc/pid/mem`.
- **secmem-proc crate (the Rust hardening library Bitwarden uses):**
  - On Windows it sets a DACL that allows only `PROCESS_QUERY_LIMITED_INFORMATION`, `PROCESS_TERMINATE` and `SYNCHRONIZE`. On Linux it disables ptrace and core dumps via prctl and rlimit, and adds debugger detection.
  - Caveats in the README: it "can by no means promise any security!" If an attacker attaches before `harden_process`, "it is game over." It "is no substitute for properly hardening your OS." Anti-tracing "can always be subverted." — [secmem-proc README](https://github.com/niluxv/secmem-proc)
- PuTTY's source has a similar DACL precaution, with a comment that it was never fully effective (startup race) and that the only real defense is keeping malware from running as the same user. — [PuTTY windows/winsecur.c mirror](https://gitea.smoothnet.org/nhyatt/putty-source/src/commit/1b2cc402448da230af90ade6331f4712f73df6b0/windows/winsecur.c)
- **Linux Yama ptrace_scope:**
  - 0 = any same-UID dumpable process. 1 = descendants only (or a `PR_SET_PTRACER`-named debugger). 2 = CAP_SYS_PTRACE only. 3 = no attach, irreversible. — [kernel.org Yama](https://www.kernel.org/doc/html/latest/admin-guide/LSM/Yama.html)
  - Ubuntu defaults to 1. That default is from background knowledge and was not verified this session.
- **Memory residue (KeePass CVE-2023-32784, May 2023):**
  - In KeePass 2.x before 2.54, the master password could be recovered (all but the first character) from a process dump, pagefile, hiberfil or full RAM dump, "even when a workspace is locked or no longer running." No code execution is needed, just a dump.
  - Cause: leftover strings from the custom password textbox. Fixed in 2.54 with different API use and random string insertion. CVSS 7.5. — [vdohney/keepass-password-dumper](https://github.com/vdohney/keepass-password-dumper); [Tenable CVE-2023-32784](https://tenable.com/cve/CVE-2023-32784); [Infosecurity Magazine](https://www.infosecurity-magazine.com/news/keepass-flaw-exposes-master/)
- age-plugin-tpm warns that a PIN passed via `AGE_TPM_PIN` "can be read from `/proc/$PID/environ`." Environment variables are visible to same-user processes. — [age-plugin-tpm README](https://github.com/Foxboron/age-plugin-tpm)

### Inferences
- **Windows DACL hardening is speed-bump grade against same-user code.** The process owner has implicit READ_CONTROL and WRITE_DAC on objects it owns (standard Windows owner semantics, background knowledge). Same-user malware can therefore usually open the process with WRITE_DAC, put back a permissive DACL, then open with VM_READ. Microsoft's page confirms WRITE_DAC is the right needed to change the DACL, but this session did not get a Microsoft quote on owner implicit rights. It still blocks naive stealers that just call `OpenProcess(PROCESS_VM_READ)`, which has real value against commodity malware. Document it as "raises the bar."
- **Linux `PR_SET_DUMPABLE=0` is stronger** than the Windows DACL against same-UID attackers, because a normal process can't undo it on another process. Note that Tauri/WebKitGTK helper processes are separate processes and need their own hardening, and the key should never enter the webview process.
- **Key practical rules for KeyHandoff:**
  1. Keep the vault key and decrypted API keys only in the Rust core process, never in the webview/JS.
  2. Keep them encrypted at rest in memory (in the style of `secure_memory`: AES-GCM under a key held in `memfd_secret`, keyctl or `CryptProtectMemory`), and decrypt per use.
  3. Zeroize (Rust `zeroize`, `secrecy`), `mlock`, and disable core dumps.
  4. Use short auto-lock and re-lock on sleep, screen lock or user switch.
  5. Require a per-use Hello or polkit confirmation for high-value actions (reveal or copy a key, Safe Copy, send). These are UV prompts, not crypto, but they stop silent automation via input injection only if the prompt is OS-drawn on the secure desktop.
  6. Avoid passing secrets via env vars or argv.
- Protected Process Light, VBS enclaves and similar mechanisms are not available to normal third-party desktop apps without special signing. Treat them as out of reach. This is background knowledge; see Gaps.
- **UI spoofing:** Windows Hello's credential UI is OS-drawn, but malware can draw a look-alike to harvest a PIN. With a TPM-bound Hello PIN, the PIN alone is useless off-device but usable on-device. Docs should warn that a prompt appearing when you didn't act is an attack.

### Gaps
- `SetProcessMitigationPolicy` options (for example, `ProcessSignaturePolicy` MicrosoftSignedOnly or `ProcessDynamicCodePolicy`) were not researched with sources. They mainly block DLL injection of unsigned code into your own process. Whether they work with WebView2 in a Tauri app is unverified.
- No Microsoft source was retrieved stating owner implicit WRITE_DAC/READ_CONTROL on process objects. The inference above should be checked (Microsoft's "Owner of a securable object" doc, or an empirical test).
- No writeup was found specifically on screen capture or input injection defenses for password managers. `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` is what Signal's "Screen security" uses ([Signal blog](https://signal.org/blog/signal-doesnt-recall/)), but its effect against malware capture was not verified.
- Wayland vs X11 differences (X11 allows any client to keylog or screenshot) are relevant to the Linux docs but were not sourced here.

## 4. The best software fallback: Argon2id parameters (2025–2026), passphrase guidance, how products pick KDF params, and offline cracking cost

### Takeaway
When no hardware is available, the vault key should be wrapped by Argon2id over a user passphrase. Use parameters well above OWASP's floor (19 MiB, t=2, p=1). Desktop apps can afford RFC 9106's memory-constrained option (64 MiB, t=3, p=4) or more; Bitwarden ships exactly that, and up to 256 MiB–1 GiB is reasonable on desktops. With roughly hundreds to two thousand guesses per second per high-end GPU at desktop parameters, a 4-word random passphrase (about 52 bits) costs tens of thousands of GPU-years. A typical human password (about 40 bits) falls to a modest GPU cluster in days. The passphrase must therefore be generated (diceware, 5–6 words recommended) or strength-checked (zxcvbn).

### Cited Findings
- **OWASP minimums for Argon2id:** m=47104 (46 MiB) t=1 p=1; m=19456 (19 MiB) t=2 p=1; m=12288 t=3; m=9216 t=4; m=7168 t=5 (all p=1). These are equivalent-strength minimums for server password storage. PBKDF2-HMAC-SHA256: 600,000 iterations. scrypt: N=2^17 r=8 p=1. — [OWASP Password Storage Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)
- **RFC 9106 §4:**
  - First recommended option: Argon2id t=1, p=4, m=2 GiB, 128-bit salt, 256-bit tag. Second (memory-constrained): Argon2id t=3, p=4, m=64 MiB.
  - Procedure: set m to the maximum affordable, then raise t within the time budget. Use Argon2id if unsure. A 128-bit salt fits all uses.
  - Example: 4 GiB with 8 lanes at about 0.5 s for backend authentication. — [RFC 9106](https://www.rfc-editor.org/rfc/rfc9106.html#section-4)
- **Bitwarden defaults:** Argon2id 64 MiB, 3 iterations, 4 threads ("above current OWASP recommendations"). The PBKDF2 default is 600,001 iterations since 2023.2.0, with a warning below 600,000. iOS autofill memory limits cap practical memory (warnings above roughly 48 MB; sources disagree on the threshold). Changing KDF logs out all clients. — [Bitwarden Help: KDF algorithms](https://bitwarden.com/help/kdf-algorithms/) (via [PDF](https://bitwarden.com/pdf/help-kdf-algorithms.pdf))
- **Cracking speed data points (inconsistent; parameters often unstated):**
  - hashcat mode 34000 (Argon2) release-note benchmark of about 1,703 H/s on an RTX 4090 vs 92 H/s on a Ryzen 9 9900X, parameters not stated in the snippet. — [hackerdna hashcat guide (secondary)](https://hackerdna.com/blog/how-to-use-hashcat)
  - Specops: 8× RTX 5090 at about 490 H/s on Argon2id; mdxfind up to 730 H/s on an EPYC CPU (page returned 403; numbers via search summary only). — [Specops blog](https://specopssoft.com/blog/does-argon2-mean-password-uncrackable/)
  - Another 2026 comparison claims about 1,900 H/s on an RTX 4090 (low-quality source). — [shattered.io](https://shattered.io/pbkdf2-vs-argon2-gpu-cracking-2026/)
  - These conflict by about 10×, most likely because the m and t settings differ.
- 1Password: account passwords average about 40 bits, which is the reason for the 128-bit Secret Key. — [1Password Support](https://support.1password.com/secret-key-security/)

### Inferences
- **Worked cracking estimate** (arithmetic, assuming about 1,000 guesses/s per GPU at 64 MiB/t=3; expected cost = half the keyspace):
  - 40-bit password: 5.5×10^11 guesses ≈ 17 GPU-years, so about 6 days on 1,000 GPUs. Feasible for a motivated attacker.
  - 4-word diceware (7776^4 ≈ 3.7×10^15, 51.7 bits): about 58,000 GPU-years.
  - 5-word diceware (64.6 bits): about 4.5×10^8 GPU-years. Out of reach.
  - 6-word diceware (77.5 bits): far beyond.
  - Raising memory from 64 MiB to 256 MiB cuts GPU throughput roughly in proportion (memory bandwidth and capacity bound), adding about 2 bits. The passphrase matters far more than parameter tuning.
- **Recommendation for KeyHandoff fallback:**
  - Argon2id, m = 256 MiB (or the largest that unlocks in about 0.5–1 s on the slowest supported machine, never below 64 MiB), t ≥ 3, p = 4, 16-byte random salt, 32-byte output used as a KEK (via HKDF) to AEAD-wrap the random vault key.
  - Store the parameters in the vault header so they can be raised later.
  - Benchmark at setup like KeePassXC's "decryption time" slider (KeePassXC behavior is from background knowledge, not verified this session).
  - Generate a 5- or 6-word EFF diceware passphrase by default. If the user types their own, reject it below about 60–64 bits of zxcvbn-estimated guesses.
  - Since the fallback is the full key in offline-attack terms, combine it with whatever device secret exists (DPAPI or Secret Service blob) as a second input. That way stealing the vault file alone isn't enough, though same-user malware gets the DPAPI half for free.
- Argon2id does nothing against keylogging the passphrase as it is typed (section 2).

### Gaps
- No authoritative, parameter-specific Argon2id GPU benchmark (hashcat's official mode 34000 numbers with m/t/p) was retrieved. The rates above are rough.
- zxcvbn / EFF wordlist guidance and NIST SP 800-63B-4 (2025) passphrase guidance were not fetched this session.
- KeePassXC KDF defaults and its benchmark-to-target-time approach were not verified from source.
- GPU cloud pricing for converting GPU-years to dollars was not researched.

## 5. Combining factors and recovery: TPM + PIN, passphrase + security key, recovery codes, and avoiding lockout when hardware fails or the TPM is cleared

### Takeaway
Every hardware-bound key can disappear: TPM clears, firmware updates that change TPM state, Windows Hello container resets, a lost YubiKey, a new motherboard. Products that bind to hardware therefore always keep a non-hardware root. That is a master password (Bitwarden, KeePassXC, 1Password) plus, for 1Password, a printed 128-bit Secret Key in an Emergency Kit. Hardware only wraps a cached copy for convenience and gating. A TPM PIN is safe even when short only because the TPM's dictionary-attack lockout rate-limits guesses. The right design is envelope encryption: one random vault key, wrapped separately by each factor (hardware or Hello, passphrase, recovery code), so losing any one factor is survivable.

### Cited Findings
- A TPM clear or firmware update can invalidate the Windows Hello PIN, because its private key lives in the TPM. The fix is re-provisioning (remove and re-add the PIN, or reset the NGC container). Microsoft notes that clearing the TPM may force reconfiguring Windows Hello, and that clearing destroys keys bound to the TPM and the data they protect. An April 2025 update (KB5055523) broke PIN and face sign-in after "Reset this PC" on some devices (fixed in KB5055627). — [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/a/8017303); [Woshub](https://woshub.com/pin-isnt-available-signing-windows/); [Flamingo](https://www.flamingo.run/blog/your-pin-isnt-available)
- 1Password's Secret Key cannot be recovered by 1Password. Copies live in the Emergency Kit, on signed-in devices, and in encrypted device backups or keychains. It "is not a backup code and can't reset a forgotten account password." — [1Password Support](https://support.1password.com/secret-key-security/)
- 1Password always falls back to the account password when Hello fails or the confirm interval lapses. — [1Password Support](https://support.1password.com/windows-hello/)
- Bitwarden's Windows code lazily migrates V1 to V2 envelopes on unlock and notes "A migration failure must not fail the unlock." It never caches "unavailable" for Hello, "so that newly connected devices (e.g. YubiKey) are detected on the next poll." — [bitwarden windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs)
- ssh-tpm-agent: "dictionary attack protection from the TPM allows you to use low entropy PINs instead of passphrases." Keys are created in the TPM, saved as sealed `.tpm` files, and "bound to the hardware they are produced on and can't be transferred." — [Foxboron/ssh-tpm-agent](https://github.com/Foxboron/ssh-tpm-agent)
- Signal shows the lockout risk of OS-store-only wrapping: a keyring backend switch (kwallet5 to kwallet6) left users unable to decrypt their database, and the workaround was losing local history. — [Gentoo Forums](https://forums.gentoo.org/viewtopic-p-8841636.html)

### Inferences
- **Recommended KeyHandoff structure:** a random 256-bit vault key (VK) with independent wrappings:
  - W_hw: Windows Hello sign-path, TPM2 sealed with a PIN on Linux, or FIDO2 hmac-secret or YubiKey PIV.
  - W_pass: Argon2id passphrase.
  - W_rec: a generated recovery code of 128 bits or more, shown once and meant to be printed, like 1Password's Emergency Kit.
  - Any one wrapping unlocks. Hardware loss requires W_pass or W_rec, then re-enrolling hardware.
  - Keep the DPAPI or Secret Service blob only as an optional "remember on this computer" tier, labeled honestly.
- **For "passphrase AND security key" (two-factor at rest):** derive KEK = HKDF(Argon2id(pass) ‖ hmac-secret-output). The recovery code must still be a separate one-factor path, or the user is locked out when the key is lost.
- Never bind TPM sealing to PCRs that change with BIOS updates (PCR 0/2/7) for a user-data vault. Use a PIN or authValue policy plus the TPM's DA lockout instead. BitLocker-style PCR binding is why firmware updates cause recovery prompts. That BitLocker behavior is background knowledge and was not sourced this session.
- Detect hardware-wrap failure, fall back cleanly to passphrase or recovery, and re-enroll automatically (as Bitwarden does). Warn the user before TPM-clearing operations where possible.

### Gaps
- Bitwarden's account recovery (admin password reset, emergency access) and Bitwarden's recovery code (which is a 2FA recovery code, not a vault key) were not sourced this session.
- No source was retrieved on Windows Hello key behavior across Hello PIN reset (whether `KeyCredentialManager` keys survive a PIN change: they should; PIN reset may not).
- Real TPM DA lockout defaults (for example, Windows default threshold and recovery time) were not sourced.

## 6. Tools (age-plugin-tpm, age-plugin-yubikey, ssh-tpm-agent, sops+age, git-credential-manager, gh, aws-vault): how they gate keys and lessons

### Takeaway
Developer secret tools split cleanly. Hardware plugins (age-plugin-yubikey, age-plugin-tpm, ssh-tpm-agent) keep the private key in hardware and gate use with a PIN or touch, sometimes cached per session. Credential helpers (gh, git-credential-manager, aws-vault) default to the OS store (wincred/DPAPI, Keychain, Secret Service), so any same-user process can read them, and some silently fall back to plaintext. KeyHandoff should copy the hardware-plugin model, make any downgrade loud, and avoid passing PINs via environment variables.

### Cited Findings
- **age-plugin-yubikey:**
  - The secret key material is stored in the YubiKey, and the identity file holds only the info needed to find which YubiKey key to use. It has `--pin-policy` and `--touch-policy` flags.
  - With PIN policy `once`, the plugin avoids soft-resetting the YubiKey so the PIN cache persists across decryptions (not on YubiKey 4). The cache ends on unplug, on use of another applet such as FIDO2, or on generating a new identity.
  - It forces changing a default PIN (and sets the PUK to the same value) and replaces the default management key with a random one stored in PIN-protected metadata. — [str4d/age-plugin-yubikey](https://github.com/str4d/age-plugin-yubikey)
- **age-plugin-tpm:** keys are "created on the TPM, sealed outside of it," with TPM session encryption. `--pin` requires a PIN for encrypt and decrypt. `AGE_TPM_PIN` exists for non-interactive use but is readable from `/proc/$PID/environ`. Recipients use the `age1tag1` prefix. — [Foxboron/age-plugin-tpm](https://github.com/Foxboron/age-plugin-tpm)
- **ssh-tpm-agent:** TPM-created keys bound to the device, TPM 2.0 Key Files (go-tpm-keyfiles), low-entropy PINs made safe by TPM dictionary-attack protection. Packaged in Debian (0.8.0). — [Foxboron/ssh-tpm-agent](https://github.com/Foxboron/ssh-tpm-agent); [Debian package](https://packages.debian.org/forky/ssh-tpm-agent)
- **gh CLI:** the token "will be stored securely in the system credential store." If that's unavailable, "gh will fallback to writing the token to a plain text file." `--insecure-storage` saves plaintext deliberately. — [gh auth login manual](https://cli.github.com/manual/gh_auth_login)
- **git-credential-manager:**
  - Windows: `wincredman` (default; doesn't work over SSH), `dpapi` (files in `%USERPROFILE%\.gcm\dpapi_store`), `plaintext` ("not a secure method").
  - macOS: `keychain` (default), `gpg` (pass-compatible), `cache` (in memory, 900 s default), `plaintext`.
  - Linux: no default store; `secretservice`, `gpg`, `cache`, `plaintext`. — [GCM credstores.md](https://github.com/git-ecosystem/git-credential-manager/blob/main/docs/credstores.md)
- **aws-vault (99designs/keyring; the maintained fork is ByteNess):**
  - Backends: keychain, wincred, secret-service, kwallet, keyctl, pass, passage, file. The default is wincred on Windows, keychain on macOS, and Secret Service on Linux, with fallback through kwallet, keyctl, pass, passage and file.
  - The `file` backend uses passphrase-based JWE, one file per credential. Selection is via `--backend` or `AWS_VAULT_BACKEND`. — [ByteNess/aws-vault README](https://raw.githubusercontent.com/ByteNess/aws-vault/main/README.md); [99designs/keyring](https://upd.dev/99designs/keyring)

### Inferences
- **Lessons for KeyHandoff:**
  1. The YubiKey "PIN once per session plus optional touch per use" model maps well onto "unlock once, confirm per Safe Copy."
  2. Silent plaintext fallback (gh) is the anti-pattern. Any degraded tier should be visible in the UI and the docs.
  3. GCM's `cache` (memory-only, timed) is the CLI analog of the ephemeral polkit tier.
  4. aws-vault's ordered backend fallback is a decent template for tier selection, but it should prefer hardware tiers first.
  5. Never accept PINs via env vars.
- **sops + age:** sops can encrypt to age recipients, including plugin recipients such as `age1yubikey...` or `age1tag1...`. A team "key handoff" could use age recipients for hardware-bound decryption. Whether sops invokes age plugins natively depends on the sops version; see Gaps.

### Gaps
- sops' age plugin support (which version added plugin identities) was not verified this session.
- age-plugin-yubikey's default PIN and touch policies (believed: PIN "once", touch "always"), from background knowledge, are not confirmed from the README text that was fetched.
- age-plugin-tpm key type (believed P-256), experimental status and backup guidance were not stated on the fetched page.
- aws-vault's "file" backend KDF parameters were not checked.
