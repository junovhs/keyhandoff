# Windows: hardware (TPM) key protection gated by a user-presence prompt

Scope: how a Rust + Tauri desktop app (unpackaged Win32 today) can replace DPAPI (CryptProtectData, current user) wrapping of a 32-byte vault master key with a TPM-backed key that only works after a Windows Hello or PIN prompt, once per session with an idle timeout. Researched 2026-10-31. Primary sources: Microsoft Learn, the windows-rs and microsoft/VbsEnclaveTooling repos, and the shipping source of Bitwarden desktop_native (Rust) and KeePassXC.

## 1. Windows Hello KeyCredentialManager: deriving a wrapping key from a signature

### Takeaway
KeyCredentialManager only signs. The working pattern, shipped by KeePassXC and by Bitwarden's Rust desktop_native code, is: create a named Hello key (RSA-2048), sign a stored random challenge with RequestSignAsync (this shows the Hello prompt), take SHA-256 of the signature (or a KDF over it), and use that as the AES/ChaCha key that wraps the secret. This depends on the signature being deterministic, and RSA PKCS#1 v1.5 signing is deterministic. Microsoft does not document which padding Hello uses, though, so treat that as an assumption and check it at runtime. The API works from unpackaged Win32 apps. Its real problems are focus (no window handle in the classic API, and face unlock fails if the prompt is not in front) and the fact that any same-user process can open the same credential by name.

### Cited Findings
**Docs: key type and API surface**
- `KeyCredentialManager.RequestCreateAsync(name, option)` "generates a new RSA 2048-bit key credential" and "Creates a new key credential for the current user and application." — [MS Learn RequestCreateAsync](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.keycredentialmanager.requestcreateasync?view=winrt-26100)
- A second overload exists only for SDK monikers winrt-26100, winrt-28000 and insider: `RequestCreateAsync(name, option, algorithm, message, KeyCredentialCacheConfiguration, WindowId, ChallengeResponseKind, AttestationChallengeHandler)`. Its parameters have no descriptions on the page. — [MS Learn RequestCreateAsync](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.keycredentialmanager.requestcreateasync?view=winrt-26100)
- `KeyCredential.RequestSignForWindowAsync(WindowId window, IBuffer data)` is documented only for monikers winrt-26100 and winrt-28000, so it needs the Windows 11 24H2-era SDK and is not on Windows 10. The page has no remarks. — [MS Learn RequestSignForWindowAsync](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.keycredential.requestsignforwindowasync)
- windows-rs master (release tag 74, 2026-09-03) now binds these:
  - `KeyCredential::RequestSignForWindowAsync(WindowId, IBuffer)` through interface `IKeyCredentialWithWindow`, gated on cargo features `Storage_Streams` + `UI`
  - `KeyCredentialManager::RequestCreateForWindowAsync(WindowId, name, option)` through `IKeyCredentialManagerCreateWithWindowStatics`, gated on `UI`
  - `RequestCreateAsync2(...)` with cache configuration and WindowId, and `OpenAsync2(name, ChallengeResponseKind, AttestationChallengeHandler)`
  - `KeyCredential::RequestDeriveSharedSecretAsync(WindowId, message, encryptedRequest)` and `RetrieveAuthorizationContext` through `IKeyCredential2`
  - `KeyCredentialCacheConfiguration::CreateInstance(KeyCredentialCacheOption, TimeSpan timeout, u32 usageCount)` and `KeyCredentialManager::GetSecureId()`

  — [windows-rs Security/Credentials/mod.rs](https://github.com/microsoft/windows-rs/blob/master/crates/libs/windows/src/Windows/Security/Credentials/mod.rs)
- Microsoft's own VBS Enclave SDK said windows-rs did not yet carry the "User Bound Key / KeyCredentialManager APIs" as of 2026-01-15 (Windows SDK 10.0.26100.7463), so it shipped a stopgap bindings crate, `userboundkey-kcm`. — [microsoft/VbsEnclaveTooling userboundkey-kcm readme](https://github.com/microsoft/VbsEnclaveTooling/tree/main/rust/sdk/crates/libs/userboundkey-kcm). The windows-rs master listing above suggests that gap has since closed. I did not check which crates.io `windows` version first carries them.

**Determinism of the signature**
- RSASSA-PKCS1-v1_5 has no random input: the same key and message always give the same signature. RSASSA-PSS uses a random salt and is not deterministic. — [RFC 8017 §8.1–8.2, §9.1–9.2](https://www.rfc-editor.org/rfc/rfc8017)
- KeePassXC PR #7384 states the assumption directly: the "implementation assumes the signature is not randomized (RSA PKCS#1 v1.5/2.0) i.e. deterministic which is the case at the moment. This can change in the future implementations of WindowsHello." — [KeePassXC PR #7384](https://github.com/keepassxreboot/keepassxc/pull/7384)
- The Microsoft tutorial for verifying Hello signatures on a server does not name the padding anywhere I could find. I found no Microsoft statement that Hello signatures are PKCS#1 v1.5 or guaranteed deterministic. — [MS Learn Windows Hello login service tutorial](https://learn.microsoft.com/en-us/windows/apps/develop/security/windows-hello-auth-service)

**KeePassXC (C++, `src/quickunlock/WindowsHello.cpp`)**
- Key name `keepassxc_winhello`. It calls `RequestCreateAsync(name, FailIfExists)`, falls back to `OpenAsync` on `CredentialAlreadyExists`, then `Credential().RequestSignAsync(challenge)`.
- The challenge is random, sized to the AES-256-GCM IV, and doubles as the IV. The AES-256-GCM key is SHA-256 of the signature. The blob (challenge || ciphertext) is held only in an in-memory map keyed by database UUID.
- Focus workaround: a 500 ms timer runs `FindWindowA("Credential Dialog Xaml Host")` then `SetForegroundWindow`, with up to 3 retries. `Userconsentverifierinterop.h` is included but unused.
- No attestation or TPM check.

— [KeePassXC WindowsHello.cpp](https://github.com/keepassxreboot/keepassxc/blob/develop/src/quickunlock/WindowsHello.cpp)

**Bitwarden (Rust, `apps/desktop/desktop_native/biometric/src/windows.rs`, current main)**
- It has two paths:
  1. A "UV path": the key is held in DPAPI-protected secure memory, and the Hello prompt is only a yes/no that releases it.
  2. A "Sign path": it "signs a per-user challenge, which produces a deterministic signature which is hashed to obtain a key", which encrypts and persists the vault user key.
- The module's own threat note: "Since the keychain can be accessed by all user-space processes, the challenge is known to all userspace processes. Therefore, to circumvent the security measure, the attacker would need to create a fake Windows-Hello prompt, and get the user to confirm it."
- It also says "Further process isolation is needed here so that code cannot be injected into the running process, which may circumvent DPAPI."
- Code: `KeyCredentialManager::RequestCreateAsync(CREDENTIAL_NAME, FailIfExists)` → `OpenAsync` → `credential.RequestSignAsync(CryptographicBuffer::CreateFromByteArray(challenge))` → `WindowsHelloPrf::derive_from_signature(sig)`. Each enrollment gets a fresh random challenge.
- While the prompt is up, a background thread calls `focus_security_prompt()` every 500 ms. That is `FindWindowA("Credential Dialog Xaml Host")` plus forced-foreground hacks, and the comment says the prompt "NEEDS to be focused instantly, or it will error, but it does not focus itself."

— [bitwarden/clients biometric/src/windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs); [windows_focus.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows_focus.rs)
- Bitwarden's Windows dependency features: `Foundation`, `Security_Credentials_UI`, `Security_Cryptography`, `Storage_Streams`, `Win32_Foundation`, `Win32_Security_Credentials`, `Win32_System_Threading`, `Win32_System_WinRT`, `Win32_UI_Input_KeyboardAndMouse`, `Win32_UI_WindowsAndMessaging`, plus `windows-future`, `sha2` and `chacha20poly1305`. — [bitwarden biometric Cargo.toml](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/Cargo.toml)
- Earlier history, PR #14953 (merged 2025-07-21):
  - Face unlock failed when the RequestSignAsync prompt was behind other windows, with error `0x80098044` (WINBIO_E_INVALID_TICKET). The author notes KeePassWinHello sees the same error.
  - The fix switched to the user-consent API for the prompt and removed the Hello-signed "OS key half", so Hello became a consent prompt rather than a crypto gate.
  - The current main shown above has re-added a sign path (V2 entries, with legacy V1 migration).

  — [bitwarden/clients PR #14953](https://github.com/bitwarden/clients/pull/14953)

**Other Rust code using RequestSignAsync (GitHub code search)**
- [nullpo-head/WSL-Hello-sudo `win_hello_bridge/src/authenticator.rs`](https://github.com/nullpo-head/WSL-Hello-sudo)
- [Aalivexy/bwbio](https://github.com/Aalivexy/bwbio). This is an unofficial Bitwarden native-messaging host that "calls Windows Hello through CNG and TPM" and claims better prompt foregrounding.
- Several Tauri apps: `fwdai/rowel src-tauri/src/secure_store.rs`, `SinaXhpm/Submarine src-tauri/src/quick_unlock.rs`, `alexandrosnt/Reach src-tauri/src/vault/biometric.rs`, `goduni/unissh client/src-tauri/src/biometric/windows.rs`.

— GitHub code search, query `RequestSignAsync language:rust`, run 2026-10-31 through `gh api search/code`. I did not review the quality of these.

**Unpackaged and background use**
- An unpackaged probe process successfully called `KeyCredentialManager.OpenAsync` and `RequestSignAsync` on a credential created by another app, and decrypted that app's data after the Hello prompt. — [MS Q&A 5912130](https://learn.microsoft.com/en-us/answers/questions/5912130/what-is-the-security-boundary-of-windows-hello-key). KeePassXC and Bitwarden also ship this API in unpackaged or Electron Win32 apps.
- Contradiction: a Microsoft external-staff moderator claimed Hello key credential calls from unpackaged Win32 apps fail with `0x80073D54` (no package identity) and recommended packaging. — [MS Q&A 6016919](https://learn.microsoft.com/en-us/answers/questions/6016919/how-do-i-construct-encryptedrequest-for-keycredent). This contradicts the shipping apps above. It probably applies only to the new 26100 overloads or the VBS-enclave flow, but that is unverified.

### Inferences
- A sound derivation is `wrap_key = HKDF-SHA256(ikm = signature, salt = random per-vault salt, info = "keyhandoff-vault-wrap-v1")`. Store salt and challenge next to the wrapped master key.
- Stored-key self-check: keep an AEAD tag, or a key-check value, over the wrapped key. If Windows ever switches to a randomized padding (PSS), unwrapping fails cleanly instead of corrupting data.
- Always keep a recovery path: a passphrase-wrapped copy of the master key, or the export/recovery file. A Hello reset, PIN reset, TPM clear, or a `DeleteAsync` by malware destroys the Hello key permanently. Malware can call `DeleteAsync` by name, so this is a denial-of-service vector.
- RequestSignAsync takes no message string, so the classic prompt cannot say what is being unlocked. The 26100 RequestCreateAsync overload has a `message` parameter. UserConsentVerifier has always taken one.
- For a Tauri app, get the HWND from the Tauri window and use the WindowId-taking APIs (`RequestSignForWindowAsync`, `RequestCreateForWindowAsync`) when `ApiInformation::IsMethodPresent` says they exist. Otherwise fall back to the FindWindow/SetForegroundWindow hack.
- Prompt branding is unknown (see Gaps). Assume the prompt does not reliably show the app's name.

### Gaps
- No Microsoft doc states the signature padding or a determinism guarantee. It must be tested on target machines (Windows 10 22H2, Windows 11 23H2/24H2/25H2).
- I found no documentation of what text or app name the RequestSignAsync prompt shows for an unpackaged exe.
- Not found: how to build a `Windows.UI.WindowId` from an HWND in plain windows-rs, without the Windows App SDK `GetWindowIdFromWindow`. Microsoft's VBS sample just takes a caller-supplied u64.
- Not found: whether `RequestSignForWindowAsync` fixes the `0x80098044` face-unlock focus failure. It is plausible but untested.
- Not checked: 1Password's Windows Hello design. The community thread and whitepaper were not retrievable this session.

## 2. Is a Hello key always TPM-backed, and can the app tell?

### Takeaway
No. Without a usable TPM, Windows Hello protects keys "with software-based techniques". The app can ask with `KeyCredential.GetAttestationAsync()`: `Success` means a TPM attestation exists, and `NotSupported` means none. Some real TPMs (AMD fTPM, VMware vTPM) reportedly also return no attestation support, so a strict "require attestation" policy will lock out some legitimate machines.

### Cited Findings
- "On a system that has a TPM, the TPM can protect the key. If a system doesn't have a TPM, software-based techniques protect the key." — [MS Learn: How Windows uses the TPM](https://learn.microsoft.com/en-us/windows/security/hardware-security/tpm/how-windows-uses-the-tpm)
- Microsoft's Hello tutorial says KeyCredentialAttestationResult values "are only provided by Windows Hello on devices that have a TPM", and shows the branches for `GetAttestationAsync()` status:
  - `Success`: `AttestationBuffer` + `CertificateChainBuffer` are available
  - `TemporaryFailure`: retry later
  - `NotSupported`: "Key created, but key attestation not supported"

  — [MS Learn Windows Hello login service tutorial](https://learn.microsoft.com/en-us/windows/apps/develop/security/windows-hello-auth-service)
- The status enum has four values: Success, UnknownError, NotSupported, TemporaryFailure. — [KeyCredentialAttestationResult](https://learn.microsoft.com/en-us/uwp/api/Windows.Security.Credentials.KeyCredentialAttestationResult); [mingw-w64 windows.security.credentials.idl](https://mingw.googlesource.com/mingw-w64/+/refs/heads/v14.x/mingw-w64-headers/include/windows.security.credentials.idl)
- A 1Password community thread (search snippet only, page not fetchable) reports that Ryzen fTPM and VMware vTPM made the system report no TPM attestation support, and that a discrete TPM worked on the same board. — [1Password community](https://www.1password.community/discussions/1password/tpm-for-windows-hello-after-restart/112367/replies/112376)
- TPM and attestation stop export and offline extraction, but they "might not resolve the 'Confused Deputy' problem": the TPM still serves valid requests from same-user malware. — [MS Q&A 5912130, Microsoft external staff answer](https://learn.microsoft.com/en-us/answers/questions/5912130/what-is-the-security-boundary-of-windows-hello-key)

### Inferences
- Local check: treat `GetAttestationAsync().Status() == Success` as "hardware-backed". Show the user "software-protected" otherwise rather than refusing.
- Verifying the attestation blob locally against the TPM AIK chain is possible but heavy. A same-user attacker cannot forge Hello's answer about its own key, so the status check is enough for UI policy.
- This holds on both Windows 10 and Windows 11. Windows 11 requires TPM 2.0 for supported installs, so software-fallback Hello keys should be rarer there (inference; TPM prevalence is covered in tpm_prevalence.md).

### Gaps
- No Microsoft doc explicitly ties `NotSupported` to the software fallback rather than to "TPM present but no attestation CA". Needs testing.

## 3. UserConsentVerifier is not a cryptographic gate

### Takeaway
`UserConsentVerifier.RequestVerificationAsync` (or `IUserConsentVerifierInterop::RequestVerificationForWindowAsync(hwnd, message)` on desktop) returns only an enum (Verified, Canceled, and so on). Nothing cryptographic is bound to the result. A same-user attacker can skip calling it, patch the app's branch, or read the secret the app releases afterwards. It is good for focus, and it carries the message text, but it is not a key gate.

### Cited Findings
- Bitwarden's own doc comment: "Get a yes/no authorization without any cryptographic backing. This API has better focusing behavior." It calls `factory::<UserConsentVerifier, IUserConsentVerifierInterop>()?.RequestVerificationForWindowAsync(GetForegroundWindow(), &HSTRING::from(message))` and maps `UserConsentVerificationResult::Verified` to true. — [bitwarden windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs)
- Bitwarden's UV path keeps the key in DPAPI-protected memory and "Further process isolation is needed here so that code cannot be injected into the running process, which may circumvent DPAPI." — [bitwarden windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs)
- With "DPAPI plus a separate consent check", approval "would be enforced in application logic rather than cryptographically". — [MS Q&A 6016919 moderator answer](https://learn.microsoft.com/en-us/answers/questions/6016919/how-do-i-construct-encryptedrequest-for-keycredent)
- When Bitwarden moved to the consent API in PR #14953, the key was no longer gated by Hello: only a client key half held in app memory remained. — [PR #14953](https://github.com/bitwarden/clients/pull/14953)

### Inferences
- KeyHandoff's current DPAPI + no prompt has the same release model as "DPAPI + UserConsentVerifier": any same-user process can call `CryptUnprotectData` on the blob directly. Adding UserConsentVerifier alone would change nothing against malware.

### Gaps
- None material.

## 4. CNG Microsoft Platform Crypto Provider (NCrypt): TPM key with a PIN

### Takeaway
The Microsoft Platform Crypto Provider (PCP, `MS_PLATFORM_CRYPTO_PROVIDER`) creates non-exportable TPM keys. An authorization value (PIN) on such a key gets the TPM's dictionary-attack lockout, which is the one thing here that makes a short PIN strong against guessing. A KeyHandoff design would be:
1. Create an RSA-2048 key in PCP with `NCRYPT_UI_POLICY` (FORCE_HIGH_PROTECTION) or with a PIN.
2. Encrypt the master key under it with RSA-OAEP.
3. Unwrap it with `NCryptDecrypt`, which needs the PIN.

The weakness: the PIN can be supplied programmatically (`NCRYPT_PIN_PROPERTY`). The prompt is not documented as running on the secure desktop, so same-user malware that keylogs or phishes the PIN can then use the key with no prompt. Lockout stops guessing, not capture.

### Cited Findings
- PCP (since Windows 8): "The operating system can load and use the keys in the TPM without copying the keys to system memory"; keys can be made non-removable.
- On dictionary-attack protection: "Keys that a TPM protects can require an authorization value such as a PIN. With dictionary attack protection, the TPM can prevent attacks that attempt a large number of guesses... After too many guesses, the TPM simply returns an error". This is stronger than software because it survives restarts, clock changes and rollback of failure counters. PINs "can be shorter and easier to remember while still providing the same level of protection".
- Summary table: with PCP, "If the machine is compromised, the private key... can't be copied off the device" and "The TPM's dictionary attack mechanism protects PIN values".

— [MS Learn: How Windows uses the TPM](https://learn.microsoft.com/en-us/windows/security/hardware-security/tpm/how-windows-uses-the-tpm)
- `NCRYPT_UI_POLICY` is `{dwVersion=1, dwFlags, pszCreationTitle, pszFriendlyName, pszDescription}`, set through `NCRYPT_UI_POLICY_PROPERTY` with `NCryptSetProperty`. Flags:
  - `NCRYPT_UI_PROTECT_KEY_FLAG` (0x1): "Display the strong key user interface as needed"
  - `NCRYPT_UI_FORCE_HIGH_PROTECTION_FLAG` (0x2): "Force high protection"
  - `NCRYPT_UI_APPCONTAINER_ACCESS_MEDIUM_FLAG` (0x8)

  The friendly name and description are shown in the strong-key dialog at creation and use. — [MS Learn NCRYPT_UI_POLICY](https://learn.microsoft.com/en-us/windows/win32/api/ncrypt/ns-ncrypt-ncrypt_ui_policy)
- High protection typically means the user "is prompted to enter a password when the key is created and whenever the key is used. The exact behavior is specified by the KSP." — [MS Learn X509PrivateKeyProtection](https://learn.microsoft.com/en-us/windows/win32/api/certenroll/ne-certenroll-x509privatekeyprotection)
- windows-rs exposes `NCRYPT_UI_FORCE_HIGH_PROTECTION_FLAG` under `windows::Win32::Security::Cryptography` (feature `Win32_Security_Cryptography`). — [windows-docs-rs constant](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Security/Cryptography/constant.NCRYPT_UI_FORCE_HIGH_PROTECTION_FLAG.html)
- Practitioner report: after too many wrong PINs, `NCryptEncrypt`/`NCryptDecrypt` return `0x80280921` (TPM lockout), and a reboot may be needed to unlock. — [Dennis Babkin, TPM blobs in Windows part 1](https://dennisbabkin.com/blog/?t=how-to-use-tpm-to-encrypt-and-decrypt-small-blobs-of-data-programmatically). This is third-party, not official.
- A systemd discussion notes that the lockout covers all TPM use (only keys without DA protection are exempt), so a DA lockout can be used as a denial of service. — [systemd #20668](https://github.com/systemd/systemd/issues/20668)

**Secure desktop and input injection**
- `CredUIPromptForWindowsCredentials` docs say a credential dialog "should be displayed on the secure desktop". This is a recommendation for that API, not a statement about the PCP or Hello prompts. — [MS Learn CredUIPromptForWindowsCredentialsW](https://learn.microsoft.com/en-us/windows/desktop/api/wincred/nf-wincred-creduipromptforwindowscredentialsw)
- Anecdotal: AutoHotkey users report that Send, SendInput, SendPlay and SendEvent no longer get digits into a waiting Windows PIN prompt, and that the failure is silent. — [AutoHotkey forum t=140045](https://www.autohotkey.com/boards/viewtopic.php?t=140045), search snippet only; the page returned 403.
- Anecdotal: a Microsoft Q&A poster says simulated clicks from Medium/High-integrity processes into secure-desktop UI started being dropped after early-2026 hardening. — [MS Q&A 5866859](https://learn.microsoft.com/en-us/answers/questions/5866859/proper-requirements-to-test-in-house-before-buying)
- The Hello prompt window ("Credential Dialog Xaml Host") is findable with `FindWindowA` and can be foregrounded by a normal user-desktop process (KeePassXC, Bitwarden). That means it lives on the normal interactive desktop, not the secure desktop. — [KeePassXC WindowsHello.cpp](https://github.com/keepassxreboot/keepassxc/blob/develop/src/quickunlock/WindowsHello.cpp); [bitwarden windows_focus.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows_focus.rs)

### Inferences
- Rust call sequence, all in `windows::Win32::Security::Cryptography` (feature `Win32_Security_Cryptography`):
  1. `NCryptOpenStorageProvider(&mut prov, MS_PLATFORM_CRYPTO_PROVIDER, 0)`
  2. `NCryptCreatePersistedKey(prov, &mut key, BCRYPT_RSA_ALGORITHM, w!("KeyHandoff-vault"), CERT_KEY_SPEC(0), NCRYPT_FLAGS(0))`
  3. Set `NCRYPT_LENGTH_PROPERTY` = 2048
  4. Set `NCRYPT_UI_POLICY_PROPERTY` with `NCRYPT_UI_FORCE_HIGH_PROTECTION_FLAG`
  5. Optionally set `NCRYPT_WINDOW_HANDLE_PROPERTY` to the Tauri HWND, so the dialog is parented
  6. `NCryptFinalizeKey(key, 0)`, which prompts the user to set the PIN
  7. `NCryptEncrypt(key, master, Some(&BCRYPT_OAEP_PADDING_INFO{ pszAlgId: BCRYPT_SHA256_ALGORITHM, .. }), out, NCRYPT_PAD_OAEP_FLAG)`
  8. Unwrap later with `NCryptDecrypt`, which prompts

  The property names come from the Microsoft key storage property identifiers list. I did not re-fetch that page this session.
- Encryption uses only the public key, so it needs no prompt. Only decryption is gated. That fits wrap-once, unwrap-per-session.
- PCP's PIN is a separate PIN the user must remember. It is not their Windows Hello PIN, and there is no face or fingerprint. This is worse UX than Hello, but its gate is the TPM's own authValue rather than the Hello service's decision.
- Same-user malware can (a) keylog the PIN if the strong-key dialog sits on the normal desktop, or (b) show its own lookalike PIN dialog. Once it has the PIN, it can set `NCRYPT_PIN_PROPERTY` and decrypt silently, as often as it wants, until the PIN changes. Hello biometrics have no equivalent "replay the factor" risk.
- Even with the Hello PIN, the same-user attacker cannot reuse a captured PIN programmatically through KeyCredentialManager; each sign shows a prompt (inference).

### Gaps
- Not found in docs: whether the PCP strong-key/PIN dialog runs on the secure desktop. It probably does not; test it.
- Not confirmed: whether PCP supports RSA-OAEP with SHA-256 on all TPM 2.0 parts. Some TPMs only support OAEP-SHA1 labels; test it. ECDH (`NCryptSecretAgreement` with a P-256 PCP key) is an alternative to test.
- Not verified this session: the Windows 10/11 DA defaults (thresholds such as 32 failures with a 2-hour heal were not confirmed).

## 5. ESS, VBS key protection, VBS enclaves / user-bound keys, Credential Guard

### Takeaway
- ESS protects the integrity of biometric matching against admin attackers. Apps inherit it automatically and there is no API for it.
- Credential Guard covers only Windows domain credentials (NTLM/Kerberos) and is not usable by apps.
- VBS key protection (CNG `NCRYPT_REQUIRE_VBS_FLAG` / `NCRYPT_PREFER_VBS_FLAG`, Windows 11 24H2+) stops admin-level key exfiltration but not key use, and it has no prompt gate.
- The only Microsoft mechanism aimed at "key usable only after Hello, only by my code" is the new Windows 11 26100.2314+ "user-bound keys": KeyCredentialManager with `ChallengeResponseKind::VirtualizationBasedSecurityEnclave`, `RequestDeriveSharedSecretAsync`, and a cache configuration. It needs a signed VBS enclave, so it is heavy and new.

### Cited Findings
- **ERNW, Black Hat 2025:** with local admin, researchers decrypted the Windows Hello for Business biometric database and injected a face, then unlocked a colleague's laptop. ESS runs the matching in VTL1 and blocks this, and it is on by default where supported. Not all PCs support it, and external cameras and readers had limited ESS support. Recommendation without ESS: use a PIN instead of biometrics. — [The Register 2025-08-07](https://www.theregister.com/2025/08/07/windows_hello_hell_no/); [Petri](https://petri.com/windows-hello-for-business-flaw-unauthorized-access/)
- **CyberArk 2021:** spoofed a USB IR camera to bypass Hello face with physical access. This affects consumer Hello too, since it shares the mechanism. — [CyberArk blog](https://www.cyberark.com/resources/threat-research-blog/bypassing-windows-hello-without-masks-or-plastic-surgery). The page now redirects; the CVE number was not re-verified.
- **VBS key protection:**
  - CNG flags `NCRYPT_REQUIRE_VBS_FLAG` (fail if no VBS) and `NCRYPT_PREFER_VBS_FLAG` (fall back to a software key), plus `NCRYPT_USE_PER_BOOT_KEY_FLAG`, on `NCryptCreatePersistedKey`. Requires VBS enabled, TPM 2.0 (or vTPM), and UEFI Secure Boot. — [MS Learn NCryptCreatePersistedKey](https://learn.microsoft.com/en-us/windows/win32/api/Ncrypt/nf-ncrypt-ncryptcreatepersistedkey)
  - The announcement (Insider build 26052): "Keys can be protected from admin-level key theft attacks"; keys live in a secure process and are TPM-encrypted at rest. "Hardware keys offer stronger protection." — [mirror of the Microsoft Tech Community post](https://thewindowsupdate.com/2024/02/08/advancing-key-protection-in-windows-using-vbs/)
  - The preview-era name `NCRYPT_USE_VIRTUAL_ISOLATION_FLAG` appears only in the reposted preview blog sample. Current docs use `NCRYPT_REQUIRE_VBS_FLAG` / `NCRYPT_PREFER_VBS_FLAG`. — [ElevenForum repost](https://www.elevenforum.com/t/advancing-key-protection-in-windows-11-using-virtualization-based-security-vbs.22327/)
- **Credential Guard:** isolates the process that hashes Windows credentials. It is aimed at AD DS deployments and does not stop key loggers. — [MS Learn: How Windows uses the TPM](https://learn.microsoft.com/en-us/windows/security/hardware-security/tpm/how-windows-uses-the-tpm)
- **VBS enclaves:** Microsoft's Windows Hello user-verification sample "demonstrat[es] user-bound encryption" and needs Windows 11 build 26100.2314+ or Server 2025, with VBS/HVCI enabled. — [MS Learn VBS enclaves](https://learn.microsoft.com/en-us/windows/win32/trusted-execution/vbs-enclaves)
- Microsoft's host code calls `KeyCredentialManager::RequestCreateAsync(name, option, algorithm, message, cacheConfiguration, WindowId, ChallengeResponseKind::VirtualizationBasedSecurityEnclave, callback)`. It maps the cache options to `KeyCredentialCacheOption::NoCache` / `CacheWhenUnlocked` with a timeout (seconds → TimeSpan) and a usage count. `credential.RequestDeriveSharedSecretAsync(windowId, message, encryptedRequest)` is commented "This prompts for the Windows Hello PIN/biometric." Key names are formatted with the user SID. — [VbsEnclaveTooling userboundkey.vtl0.cpp](https://github.com/microsoft/VbsEnclaveTooling/blob/main/src/VbsEnclaveSDK/src/veil_host_lib/userboundkey.vtl0.cpp)
- Microsoft's Rust stopgap crate `userboundkey-kcm` exposes `RequestCreateAsync2`, `OpenAsync`(ChallengeResponseKind) and `GetSecureId` for the `vbsenclave-sdk-host` crate. — [userboundkey-kcm readme](https://github.com/microsoft/VbsEnclaveTooling/tree/main/rust/sdk/crates/libs/userboundkey-kcm)
- Moderator claims (unofficial, from MS Q&A 6016919):
  - There is no public spec for `encryptedRequest`, the "supported workflow requires a VBS enclave", and `ChallengeResponseKind` has a single member, `VirtualizationBasedSecurityEnclave`.
  - Prerequisites: the MSVC enclave toolchain, `veinterop.lib` from SDK 10.0.26100.7463+, and a VBS-enclave code-signing certificate (Trusted Signing).

  — [MS Q&A 6016919](https://learn.microsoft.com/en-us/answers/questions/6016919/how-do-i-construct-encryptedrequest-for-keycredent)

### Inferences
- **ESS:** it raises the bar for faking a face or finger against admins, which helps every Hello-gated app. It does nothing against same-user malware that simply triggers a real prompt and waits for the user to approve.
- **VBS key protection** fixes the wrong problem for KeyHandoff. DPAPI's weakness is that any same-user process can use it, not that the key gets copied out. A VBS-isolated key the same user can call `NCryptDecrypt` on is no better at gating use.
- **User-bound keys + VBS enclave:** this is the only path where the unwrapped secret never enters normal process memory (VTL0) and use needs Hello. The enclave would hold the vault key and do the vault crypto itself. The costs are a signed enclave DLL (Trusted Signing VBS cert), Windows 11 24H2+ with VBS/HVCI on, and C++ or Rust-enclave tooling that is still young. Treat it as a future tier.
- **Malware loading the same enclave:** same-user malware could probably load KeyHandoff's signed enclave itself. It would still face a Hello prompt, but could then use the enclave as an oracle. Unverified.

### Gaps
- No official doc confirms the VBS-enclave requirement for `RequestDeriveSharedSecretAsync`, and none confirms whether the new cache configuration works with ordinary (non-enclave) RequestSignAsync use.
- I did not verify whether ESS covers PIN entry (as opposed to biometric matching) or puts the PIN prompt on a secure surface.
- Not checked this session: the current status of ESS external-peripheral support (it was "expected late 2025" in the 2025 coverage).

## 6. What the prompt protects against, and what it does not

### Takeaway
Against same-user malware, a Hello- or PIN-gated TPM key buys exactly one thing: no silent, at-rest unwrap. The attacker must get a real prompt approved, or catch the key after it is unwrapped. Everything after the unwrap (process memory, injection) and every social-engineering prompt is still open. Combine it with a passphrase factor, or accept the residual risk explicitly.

### Cited Findings
- KeyCredentialManager credentials are scoped to the user account for unpackaged and Full Trust apps. Another same-user process can `OpenAsync` the credential by name and, after the user approves a Hello prompt, sign with it.
  - The poster demonstrated this empirically ("RequestSignAsync returned Success (0) after Windows Hello user verification"; data decrypted).
  - A Microsoft external-staff moderator said only a real AppContainer (`uap10:TrustLevel="appContainer"`) gives PFN-scoped isolation: "If the app runs as Full Trust, cryptographic boundaries degrade to the user level."
  - The moderator endorsed a split-key design (password-derived half + Hello half): "if malware successfully triggers the Hello prompt, it only retrieves half the key."

  — [MS Q&A 5912130](https://learn.microsoft.com/en-us/answers/questions/5912130/what-is-the-security-boundary-of-windows-hello-key)
- Bitwarden's threat statement: to defeat the sign path, "the attacker would need to create a fake Windows-Hello prompt, and get the user to confirm it". Its UV path also needs "process isolation... so that code cannot be injected into the running process". — [bitwarden windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs)
- The Hello prompt window class is a well-known, findable target for automation, and fake credential dialogs are an established phishing technique. — [GitHub gist: fake credential prompt PoC](https://gist.github.com/An0nUD4Y/9efeef6773e19afc4dcb83431aacd7d1)

### Inferences
**Protected (with a Hello sign-derived key or a PCP PIN key)**
- Offline theft of the vault plus the wrapped key from disk or backup. The key cannot leave the TPM; this applies to TPM-backed keys only.
- Silent same-user unwrap while the user is away or the app is locked. This is the improvement over DPAPI.
- Brute force of a short PIN (TPM DA lockout).

**Not protected**
1. **Waiting for an unlocked session:** the malware reads the master key from KeyHandoff's memory (ReadProcessMemory or DLL injection, both allowed for the same user at the same integrity) during the session window.
2. **Prompt hijack:** the malware calls `OpenAsync(<KeyHandoff's credential name>)` + `RequestSignAsync(<challenge read from KeyHandoff's file>)`. That shows a real, identical Hello prompt, which the user approves out of habit, and the malware derives the same wrap key. The classic prompt carries no message or app identity, so the user cannot tell the two apart (see section 1, Gaps).
3. **Timing:** the malware triggers the prompt right when KeyHandoff would normally prompt, for example at app launch.
4. **PIN capture** for a PCP PIN key, followed by unlimited silent use.
5. **DoS:** `DeleteAsync` on the Hello credential, or deliberately tripping the TPM DA lockout.
6. **Input injection:** the attacker cannot approve biometrics by injection, and injecting into a PIN prompt reportedly fails on current builds (anecdotal). The PIN must be known to the attacker anyway.

**Mitigations, all partial**
- Use a random per-install credential name stored in DPAPI. This is weak: the same user can read DPAPI too.
- Keep the challenge in DPAPI rather than plaintext.
- Show KeyHandoff's own "about to ask Windows Hello" UI, so an unexpected Hello prompt looks suspicious.
- Keep the unlocked window short (idle timeout) and zeroize memory on lock.
- Offer an optional passphrase factor (split key).
- The packaged AppContainer route is impractical for a Tauri app that needs the clipboard, file and network access.

### Gaps
- No primary source found measuring how often users approve unexpected Hello prompts.

## 7. Prompt frequency patterns

### Takeaway
The shipping desktop password managers studied here use Hello as quick unlock after the first unlock with a master password, then re-prompt per lock event. Bitwarden caches the derived Hello key briefly so re-enrolling does not prompt twice. Windows 11 24H2+ adds OS-level caching on the new KeyCredentialManager overload (`KeyCredentialCacheOption::CacheWhenUnlocked` + timeout + usage count). For KeyHandoff: prompt once at session start, hold the master key in memory, lock on idle timeout, OS lock, or sleep, and prompt again on next use.

### Cited Findings
- KeePassXC: Hello-encrypted keys live only in an in-memory map (`m_encryptedKeys`). Quick unlock therefore works only within an app session, after a full unlock. — [KeePassXC WindowsHello.cpp](https://github.com/keepassxreboot/keepassxc/blob/develop/src/quickunlock/WindowsHello.cpp)
- Bitwarden: the UV path "only works after first unlock" (key held in memory). The sign path persists the key. It reuses "the Windows Hello challenge and key from the last prompt, so re-enrolling right after a biometric unlock does not prompt again". — [bitwarden windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs)
- Bitwarden disabled biometric unlock on app start on Windows in mid-2025, considering it unsafe without sandboxed storage. The later sign path appears to restore persisted unlock. — [PR #14953](https://github.com/bitwarden/clients/pull/14953)
- `KeyCredentialCacheConfiguration(cacheOption: NoCache|CacheWhenUnlocked, timeout, usageCount)` is passed to the 26100 RequestCreateAsync overload. — [VbsEnclaveTooling userboundkey.vtl0.cpp](https://github.com/microsoft/VbsEnclaveTooling/blob/main/src/VbsEnclaveSDK/src/veil_host_lib/userboundkey.vtl0.cpp); [windows-rs bindings](https://github.com/microsoft/windows-rs/blob/master/crates/libs/windows/src/Windows/Security/Credentials/mod.rs)

### Inferences
- Use the app's own cache (master key in locked, zeroize-on-lock memory) rather than relying on the OS cache. The OS cache is only on 24H2+ and is possibly enclave-only.
- Lock triggers to wire up:
  - idle timeout (user-configurable, e.g. 5 to 15 minutes)
  - `WTS_SESSION_LOCK` (`WTSRegisterSessionNotification`)
  - suspend (`PBT_APMSUSPEND`)
  - explicit lock
- Per-use prompts would be the strongest against "wait for unlock" attacks, but they are tiresome. A middle ground is to prompt per send or export of a key, only for sensitive actions.

### Gaps
- Not verified: 1Password's and Bitwarden's documented default re-prompt intervals.

## 8. Concrete Rust (windows-rs) paths and features

### Takeaway
Everything is reachable from the `windows` crate. Hello needs these features: `Security_Credentials`, `Security_Cryptography` (CryptographicBuffer), `Storage_Streams` (IBuffer), `Foundation`, `Win32_System_WinRT` (IBufferByteAccess, IUserConsentVerifierInterop), plus `UI` for WindowId on the new APIs, and `Security_Credentials_UI` for UserConsentVerifier. NCrypt/PCP needs `Win32_Security_Cryptography`. Bitwarden's `desktop_native/biometric` is the best existing Rust reference, under GPL-3.0.

### Cited Findings
- Bitwarden feature list and full Hello flow in Rust. — [bitwarden biometric Cargo.toml](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/Cargo.toml); [windows.rs](https://github.com/bitwarden/clients/blob/main/apps/desktop/desktop_native/biometric/src/windows.rs)
- windows-rs API paths:
  - `windows::Security::Credentials::{KeyCredentialManager, KeyCredential, KeyCredentialCreationOption, KeyCredentialStatus, KeyCredentialAttestationStatus, KeyCredentialCacheConfiguration}`
  - The methods `RequestCreateAsync`, `OpenAsync`, `IsSupportedAsync`, `DeleteAsync`, `RequestSignAsync`, `GetAttestationAsync`, `RequestSignForWindowAsync` (features `Storage_Streams`+`UI`) and `RequestCreateForWindowAsync` (feature `UI`)

  — [windows-rs Security/Credentials/mod.rs](https://github.com/microsoft/windows-rs/blob/master/crates/libs/windows/src/Windows/Security/Credentials/mod.rs)
- `NCRYPT_UI_FORCE_HIGH_PROTECTION_FLAG` lives in `windows::Win32::Security::Cryptography`. — [windows-docs-rs](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Security/Cryptography/constant.NCRYPT_UI_FORCE_HIGH_PROTECTION_FLAG.html)

Sketch, modelled on Bitwarden:
```rust
use windows::{core::HSTRING, Security::Credentials::*, Security::Cryptography::CryptographicBuffer};
let name = HSTRING::from("KeyHandoff-vault-<random>");
let r = KeyCredentialManager::RequestCreateAsync(&name, KeyCredentialCreationOption::FailIfExists)?.await?;
let cred = match r.Status()? {
    KeyCredentialStatus::CredentialAlreadyExists => KeyCredentialManager::OpenAsync(&name)?.await?,
    KeyCredentialStatus::Success => r,
    s => bail!("hello create failed: {s:?}"),
}.Credential()?;
// hardware check (once, at setup):
let hw = cred.GetAttestationAsync()?.await?.Status()? == KeyCredentialAttestationStatus::Success;
let sig = cred.RequestSignAsync(&CryptographicBuffer::CreateFromByteArray(&challenge)?)?.await?; // prompts
if sig.Status()? != KeyCredentialStatus::Success { bail!("cancelled") }
let mut arr = windows::core::Array::<u8>::new();
CryptographicBuffer::CopyToByteArray(&sig.Result()?, &mut arr)?; // Bitwarden uses IBufferByteAccess instead
let wrap_key = hkdf_sha256(&arr[..], &salt, b"keyhandoff-vault-wrap-v1");
```
Note: this sketch has not been compiled. Bitwarden reads the bytes through `IBufferByteAccess` (`Win32_System_WinRT`). WinRT async `.await` needs windows-future/IntoFuture, or use `.get()` on a worker thread, not the Tauri main thread.

### Inferences
- **Fallback ladder for KeyHandoff on Windows:**
  1. Hello sign-derived key: TPM-backed if attestation succeeds, otherwise labelled "software".
  2. PCP PIN key, if Hello is unavailable but a TPM exists.
  3. DPAPI, labelled "no prompt".
- Keep a passphrase recovery path in all cases.
- **Windows 10 vs Windows 11:** the classic `RequestCreateAsync`/`OpenAsync`/`RequestSignAsync`/`GetAttestationAsync` and `UserConsentVerifier` are available on Windows 10 1507+ (10240 moniker). The WindowId variants, the cache configuration and the VBS enclave/user-bound keys need the 26100 SDK at runtime (Windows 11 24H2+). Probe with `Windows::Foundation::Metadata::ApiInformation::IsMethodPresent`. VBS key protection flags are also 24H2+ (from Insider build 26052).

### Gaps
- No standalone, audited Rust crate was found for "Hello-gated key derivation". Bitwarden's module is the closest; the crates found by code search are app-internal and unreviewed.
- No end-to-end Rust sample was found for PCP + `NCRYPT_UI_POLICY` + RSA-OAEP. The call sequence above is assembled from the docs and needs testing.
