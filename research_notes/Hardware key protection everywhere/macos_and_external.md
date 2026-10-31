# Hardware-backed, prompt-gated master-key protection outside the PC TPM: (A) macOS Secure Enclave, (B) external authenticators (FIDO2 keys, passkeys, phones)

Research date: 2026-10-31. Scope: wrapping a 32-byte vault master key so that same-user malware cannot unwrap it without a user-presence prompt, from a Rust + Tauri desktop app (Windows, Linux now; macOS later).

Note on method: Primary sources fetched this session were the Microsoft `webauthn.h` header (GitHub), Apple developer docs (TN3137, "Protecting keys with the Secure Enclave", `touchIDAuthenticationAllowableReuseDuration`), the `systemd-cryptenroll` man page, Yubico's hmac-secret deep dive, the Corbado PRF support matrix (updated 2026-09-29), crates.io metadata, and the source trees of `security-framework`, `authenticator-rs`, `webauthn-authenticator-rs` and `ctap-hid-fido2`. Items not verified this session are kept in Gaps or marked as background knowledge.

---

## A1. How a Secure Enclave key wraps a 32-byte master key (SecAccessControl, ECIES/ECDH, LAContext reuse)

### Takeaway
The Secure Enclave only holds NIST P-256 keys and only does signing and ECDH, so you wrap a symmetric master key by ECIES-encrypting it to the enclave key's public half (`kSecKeyAlgorithmECIESEncryptionCofactor[VariableIV]X963SHA256AESGCM`) and decrypting with the enclave private key. The decrypt is gated by a `SecAccessControl` (for example `.privateKeyUsage | .userPresence`). For "once per session", unwrap once and keep the key in memory, or reuse one evaluated `LAContext`. `touchIDAuthenticationAllowableReuseDuration` is a different thing (see below).

### Cited Findings
- The Secure Enclave "works only with NIST P-256 elliptic curve keys. These keys can only be used for creating and verifying cryptographic signatures, or for elliptic curve Diffie-Hellman key exchange (and by extension, symmetric encryption)". Keys can't be imported; they are generated in the enclave. — [Apple: Protecting keys with the Secure Enclave](https://developer.apple.com/documentation/security/protecting-keys-with-the-secure-enclave)
- Apple's guide says the enclave supports the cofactor X9.63 SHA-256 AES-GCM ECIES algorithm. `kSecAttrTokenID = kSecAttrTokenIDSecureEnclave` makes the key generate inside the enclave, and only the enclave that created a key can use it. — [Apple: Protecting keys with the Secure Enclave](https://developer.apple.com/tutorials/data/documentation/security/protecting-keys-with-the-secure-enclave.md)
- `kSecAccessControlPrivateKeyUsage` is the constraint for Secure Enclave keys that lets the private key be used for operations inside the enclave. — [Apple: privateKeyUsage](https://developer.apple.com/documentation/security/secaccesscontrolcreateflags/privatekeyusage.md)
- Access-control flag values as exposed in Rust (`security-framework-sys`): `kSecAccessControlUserPresence = 1<<0`, `BiometryCurrentSet = 1<<3`, `DevicePasscode = 1<<4`, `Or = 1<<14`, `And = 1<<15`, `PrivateKeyUsage = 1<<30`, `ApplicationPassword = 1<<31`. — [security-framework-sys access_control.rs](https://github.com/kornelski/rust-security-framework/blob/main/security-framework-sys/src/access_control.rs)
- `touchIDAuthenticationAllowableReuseDuration` lets an auth succeed without a new prompt "if the user unlocks the device using Touch ID within the specified time interval". Its purpose is to skip a second fingerprint right after a device unlock. The default is 0 (no reuse), and it is capped by `LATouchIDAuthenticationMaximumAllowableReuseDuration`. — [Apple: touchIDAuthenticationAllowableReuseDuration](https://developer.apple.com/documentation/localauthentication/lacontext/touchidauthenticationallowablereuseduration)
- Pitfall: passing the Secure Enclave token ID to `SecKeyCreateWithData` seems to create a new key instead of restoring the old one. You keep an enclave key either as a persistent keychain item or as its enclave-encrypted data representation, which only the same enclave can use. — [Apple Developer Forums 748611](https://developer.apple.com/forums/thread/748611?page=1)
- CryptoKit (`SecureEnclave.P256`) and the Security framework are separate paths to the same enclave. There are forum threads on ECIES interop between them. — [Apple Forums 728314](https://developer.apple.com/forums/thread/728314); [Apple Forums 744854](https://developer.apple.com/forums/thread/744854)

### Inferences
- Recommended wrap design: generate a permanent SE P-256 key with access control `PrivateKeyUsage | UserPresence`. UserPresence means Touch ID or Apple Watch, falling back to the login password. `BiometryCurrentSet` is stricter: re-enrolling fingers invalidates the key, which makes it a recovery hazard. Next, run `SecKeyCreateEncryptedData(pub, ECIESEncryptionCofactorVariableIVX963SHA256AESGCM, master_key)` and store the blob on disk. To unwrap, call `SecKeyCreateDecryptedData(priv, ...)`, which triggers the prompt. The public-key encrypt needs no prompt, so re-wrapping (for example after rotating the master key) doesn't prompt either.
- The DIY alternative is ECDH (`SecKeyCopyKeyExchangeResult` with `ECDHKeyExchangeCofactorX963SHA256`) against an ephemeral key, then HKDF to an AES key. It is equivalent but needs more custom code. Prefer the built-in ECIES unless KeyHandoff wants one wrap format across platforms.
- `touchIDAuthenticationAllowableReuseDuration` is not an "authenticate once per app session" knob. It only reuses a recent device-unlock Touch ID. For once per session, either unwrap once and hold the master key in locked memory until lock or idle timeout (simplest), or keep one `LAContext`, evaluate it once, and pass it to keychain and key operations through `kSecUseAuthenticationContext`. The Rust crate exposes this as `ItemSearchOptions::local_authentication_context`.

### Gaps
- I did not fetch Apple's docs confirming that a once-evaluated `LAContext` passed through `kSecUseAuthenticationContext` suppresses the prompt for later SE-key decrypts, or how long it stays valid. This is commonly described but unverified here.
- I did not fetch the exact value of `LATouchIDAuthenticationMaximumAllowableReuseDuration`. From background knowledge it is 300 seconds; this is unverified.
- Macs without a Touch ID sensor (desktop Macs without a Touch ID keyboard) fall back to the account password for `.userPresence`. This is background knowledge, not confirmed this session.

---

## A2. Which Macs have a Secure Enclave, and how many Macs in use

### Takeaway
Apple's docs say Secure Enclave keys work on "a Mac with the Touch Bar and Touch ID or with an M1 or later processor", in practice T2 Intel Macs and all Apple silicon. macOS 26 Tahoe is the last macOS for Intel, and macOS 27 is Apple silicon only. So by the time KeyHandoff ships on macOS, nearly every Mac on a supported OS has an enclave.

### Cited Findings
- Secure Enclave key protection "requires hardware support. Only ... a Mac with the Touch Bar and Touch ID or with an M1 or later processor support this feature." — [Apple: Protecting keys with the Secure Enclave](https://developer.apple.com/documentation/security/protecting-keys-with-the-secure-enclave)
- macOS Tahoe 26 runs on all M-series Macs and "a small number of Intel-powered Macs from 2019". Apple has confirmed Tahoe is the last macOS for Intel, and macOS 27 drops Intel. Sources disagree on exactly which Intel models made the cut. — [Macworld](https://macworld.com/article/2644146/macos-26-features-latest-update-release-date-beta.html); [PetaPixel](https://petapixel.com/2025/06/10/macos-26-tahoe-is-the-end-of-the-road-for-intel-macs/); [Yahoo/How-To Geek](https://tech.yahoo.com/computing/articles/macs-compatible-macos-26-tahoe-171449672.html); [EveryMac Tahoe list](https://everymac.com/mac-answers/macos-26-tahoe-faq/macos-tahoe-macos-26-compatbility-list-system-requirements.html)
- Compatible Intel Macs reportedly get Tahoe security updates for about three more years. This is a secondary source. — [iBoysoft](https://iboysoft.com/wiki/macos-tahoe-supported-devices.html)
- Secretive notes that Macs without a Secure Enclave can use a smart card such as a YubiKey instead. This confirms that pre-T2 Macs are a real, if shrinking, case. — [Secretive README](https://github.com/maxgoedjen/secretive)

### Inferences
- The Intel Macs still supported by Tahoe are 2019–2020 models. The ones commonly listed (16" MBP 2019, 13" MBP 2020 four-port, iMac 2020, Mac Pro 2019) all have a T2 chip. If so, every Mac on macOS 26 or later has a Secure Enclave, and a Mac without one runs macOS 15 or older. Check the exact list before relying on it.
- Practical policy: require Secure Enclave on macOS, and treat non-enclave Macs like TPM-less PCs by falling back to an external authenticator (B).

### Gaps
- I found no reliable source for the Apple silicon (or T2) share of Macs in active use as of 2026. Searches returned compatibility lists, not installed-base data.

---

## A3. Requirements: signing, entitlements, data protection keychain; Rust support

### Takeaway
Secure Enclave keys need the data protection keychain. That needs keychain-access-group entitlements authorized by a provisioning profile embedded in an app bundle (a signed Developer ID app with a team ID). This is also the security win: macOS limits access to a stored key by code signature, so a same-user process from another team can't even reference the key. In Rust, `security-framework` 3.7.0 supports all of it: `Token::SecureEnclave`, `Location::DataProtectionKeychain`, `SecAccessControl`, ECIES encrypt/decrypt, key exchange and an `LAContext` hook.

### Cited Findings
- TN3137: features that need the data protection keychain include iCloud Keychain, "protecting an item with biometrics (Touch ID and Face ID)" and "protecting a key with the Secure Enclave." — [Apple TN3137: On Mac keychain APIs and implementations](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)
- TN3137: the data protection keychain "uses keychain access groups, supplemented by an optional access control object". macOS builds a program's access groups "from its code signing entitlements. These entitlements must be authorized by a provisioning profile. Your program needs an app-like bundle structure in which to embed that profile... not for command-line tools." It also says the data protection keychain "is only available in a user login context. You can't use it, for example, from a launchd daemon." — [Apple TN3137](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)
- `security-framework` (v3.7.0, 2026-02-20, about 428M downloads): `GenerateKeyOptions` has `set_token`, `set_access_control`, `set_location`, `set_label`, `set_key_type` and `set_size_in_bits`. The `Token` enum has `SecureEnclave`, which sets `kSecAttrTokenIDSecureEnclave`, and `Location` has `DataProtectionKeychain`, which sets `kSecUseDataProtectionKeychain` behind feature `OSX_10_15` on macOS. `SecKey` has `new(&GenerateKeyOptions)`, `public_key()`, `external_representation()`, `encrypt_data(Algorithm, &[u8])`, `decrypt_data(...)`, `create_signature` and `key_exchange(algorithm, public_key, requested_size, shared_info)`. `SecAccessControl::create_with_flags` and `create_with_protection(Option<ProtectionMode>, flags)` exist, and `ItemSearchOptions` has `local_authentication_context(Option<LAContext>)`. — [docs.rs GenerateKeyOptions](https://docs.rs/security-framework/latest/security_framework/key/struct.GenerateKeyOptions.html); [key.rs source](https://github.com/kornelski/rust-security-framework/blob/main/security-framework/src/key.rs); [access_control.rs](https://github.com/kornelski/rust-security-framework/blob/main/security-framework/src/access_control.rs); [item.rs](https://github.com/kornelski/rust-security-framework/blob/main/security-framework/src/item.rs); [crates.io](https://crates.io/crates/security-framework)
- The `Algorithm` enum in `security-framework-sys` includes `ECIESEncryptionCofactorVariableIVX963SHA256AESGCM`, `ECDHKeyExchangeStandard` and `ECDHKeyExchangeCofactorX963SHA256`. — [security-framework-sys key.rs](https://github.com/kornelski/rust-security-framework/blob/main/security-framework-sys/src/key.rs)
- Raw bindings exist as an alternative: `objc2-security` and `objc2-local-authentication` (v0.3.2, 2025-10-04). — [crates.io objc2-local-authentication](https://crates.io/crates/objc2-local-authentication); [crates.io objc2-security](https://crates.io/crates/objc2-security)
- Counterpoint: the `envseal` crate's macOS backend uses direct Security.framework FFI because, per its comments, the wrapper crates didn't expose SEP token, permanent storage, application tag and ECIES together. That may predate current `security-framework` releases. — [envseal secure_enclave.rs](https://docs.rs/crate/envseal/latest/source/src/vault/hardware/secure_enclave.rs)

### Inferences
- Tauri macOS builds must be a signed `.app` with a Developer ID team, a `keychain-access-groups` entitlement and an embedded provisioning profile. An ad-hoc-signed dev build won't be able to create SE keys in the data protection keychain, so plan CI signing early.
- Threat-model plus: on macOS, same-user malware can't use KeyHandoff's SE key at all unless it is signed with KeyHandoff's team ID and entitlements, and the `.userPresence` prompt is a second gate. This is stronger than Windows/Linux, where any same-user process can call the same TPM, webauthn or HID APIs. Residual risks: code injection into KeyHandoff itself (blocked by the hardened runtime and library validation), and UI spoofing at prompt time.
- `security-framework` probably doesn't expose `kSecAttrApplicationTag` or `kSecAttrIsPermanent` on `GenerateKeyOptions`. The fields above don't include them; it uses `label`. Expect to look the key up by label, or drop to `SecKey::generate(CFDictionary)` with a raw attribute dictionary. The raw path exists.

### Gaps
- I did not confirm whether `security-framework` has the non-VariableIV ECIES variant (`ECIESEncryptionCofactorX963SHA256AESGCM`). Only the VariableIV variant was confirmed by grep. Either works with the enclave per Apple, but pick one and never change it, because the formats differ.

---

## A4. How 1Password, Bitwarden, KeePassXC and Secretive do it on macOS

### Takeaway
Secretive is the cleanest public reference: non-exportable SE keys in the keychain, Touch ID or Apple Watch gating, and per-use notifications. I did not verify the internal designs of 1Password, Bitwarden and KeePassXC on macOS this session.

### Cited Findings
- Secretive stores SSH keys in the Secure Enclave ("impossible to export them, by design"). It uses Keychain APIs, so reads are limited to the bundle ID that created them. Keys can require Touch ID or Apple Watch, and it notifies on every key access. Because keys can't be exported, they can't be backed up or moved to a new Mac. If you build from source, keep a consistent bundle ID or the keychain won't find the keys. Builds have had GitHub artifact attestations since 3.0. — [Secretive README](https://github.com/maxgoedjen/secretive)

### Inferences
- The Secretive pattern applies directly to KeyHandoff on macOS. The lesson "keys can't be backed up" means the SE wrap must be one of several wraps of the master key (alongside a recovery code or a second authenticator), never the only one.

### Gaps
- 1Password's macOS Touch ID unlock: whether it uses an SE key or a biometric-ACL keychain item, and how it handles reuse. Not researched this session.
- Bitwarden desktop biometric unlock on macOS, and KeePassXC Touch ID quick unlock (background: it stores a key in the keychain with biometric ACL). Unverified this session.

---

## B1. FIDO2 hmac-secret (CTAP2) and WebAuthn PRF: how it derives a stable 32-byte secret; which keys; price

### Takeaway
`hmac-secret` is a CTAP2 extension. The authenticator holds a per-credential secret and returns HMAC-SHA-256(secret, salt), 32 bytes, for one or two 32-byte salts, after a touch (UP) and optionally a PIN or biometric (UV). WebAuthn PRF is the browser-facing name; the browser pre-hashes the salt with the "WebAuthn PRF" label. Support is near universal on modern FIDO2 keys: about 94% of 325 keys list hmac-secret, and YubiKey has supported it for a long time. A one-touch create variant (hmac-secret-mc) needs YubiKey firmware 5.8+.

### Cited Findings
- WebAuthn `prf` is the browser API name for CTAP2 `hmac-secret`. In web contexts the browser sends `SHA-256("WebAuthn PRF" || 0x00 || developerSalt)`, not the raw salt. The output is 32 bytes. Native libfido2 apps skip that prefixing, "so they must handle their own domain separation". After the secret reaches the client the authenticator is "not directly involved", so the app must run it through a KDF (HKDF) right away and wipe it. If the key is lost, data encrypted with the derived key can't be recovered; Yubico recommends multi-device envelope encryption. — [Yubico: CTAP2 hmac-secret deep dive](https://developers.yubico.com/WebAuthn/Concepts/PRF_Extension/CTAP2_HMAC_Secret_Deep_Dive.html)
- Yubico documents hmac-secret as long supported. The one-touch `hmac-secret-mc` variant (PRF output at creation) needs firmware 5.8 or later. Yubico's SDK shows how to read `AuthenticatorInfo` to confirm support. — [Yubico PRF support](https://developers.yubico.com/Passkeys/Passkey_concepts/Security_key_capabilities/Pseudo-Random_Function_PRF_Support.html); [Yubico .NET SDK hmac-secret](https://docs.yubico.com/yesdk/users-manual/application-fido2/hmac-secret.html)
- Corbado's getInfo survey of 325 keys: about 94% list hmac-secret, and a browser won't request PRF from a key that doesn't list it. — [Corbado: security key capabilities](https://www.corbado.com/blog/security-key-capabilities.md)
- Corbado's categories: some CTAP 2.0/2.1 keys give PRF only if it was requested at creation. Newer tokens (and iCloud Keychain, GPM, Samsung Pass) give PRF on get even if it wasn't requested at create. Full CTAP 2.2 gives PRF output on create too. — [Corbado PRF guide](https://www.corbado.com/blog/passkeys-prf-webauthn)
- `ctap-hid-fido2` lists tested keys: YubiKey (incl. firmware 5.8), YubiKey Bio, Feitian ePass FIDO (A4B), BioPass K27 and AllinPass K33, SoloKey, Nitrokey FIDO2, OpenSK and Idem Key. — [ctap-hid-fido2 README](https://github.com/gebogebogebo/ctap-hid-fido2)
- `systemd-cryptenroll --fido2-device` needs a token with hmac-secret and notes that "most FIDO2 keys, including YubiKeys, do". — [systemd-cryptenroll(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-cryptenroll.html)
- privacyIDEA's FIDO2 compatibility list names Google Titan (USB-A/NFC, USB-C/NFC), Feitian ePass K9/K40 and Token2 T2F2. It covers FIDO2 generally, not PRF. — [privacyIDEA supported tokens](https://privacyidea.readthedocs.io/en/latest/tokens/supported_tokens.html)

### Inferences
- Native app salt design: use a fixed app-specific 32-byte salt, or a random per-vault salt stored next to the credential ID. HKDF the output into a key-encryption key, then AES-GCM-wrap the 32-byte master key. The salt and credential ID aren't secret; the security rests on the per-credential secret staying inside the token. Use a non-discoverable credential (credential ID stored on disk) so no resident slot is used. Older keys have limited resident slots, and non-discoverable credentials work on more keys.
- Because some older keys only give PRF if hmac-secret was enabled at creation, always set `hmac-secret: true` (Windows: `bEnablePrf`) on MakeCredential.

### Gaps
- Prices: I found no current price source. Background knowledge only, unverified: entry FIDO2 keys (Yubico Security Key NFC, Token2, Feitian) about US$20–30; YubiKey 5 series about US$50–90; Google Titan about US$30–35.
- PRF confirmed by testing for Titan, Feitian, Token2 and Nitrokey 3 specifically: not found (Corbado's survey is aggregate).
- Background from the CTAP 2.1 spec, not re-fetched: hmac-secret derives different outputs with and without UV (CredRandomWithUV vs CredRandomWithoutUV), so the wrap must always be done in the same UV mode. Salts travel encrypted under the PIN/UV protocol shared secret. Check against the [CTAP 2.1 spec](https://fidoalliance.org/specs/fido-v2.1-ps-20210615/fido-client-to-authenticator-protocol-v2.1-ps-20210615.html).

---

## B2. Using hmac-secret/PRF from a native desktop app: Windows webauthn.dll; Linux/macOS direct HID and Rust crates

### Takeaway
On Windows, a non-admin app must go through `webauthn.dll`, which can't reach raw HID. It does expose hmac-secret/PRF on GetAssertion (`pHmacSecretSaltValues`, options v6) and can take raw 32-byte hmac-secret salts with `WEBAUTHN_AUTHENTICATOR_HMAC_SECRET_VALUES_FLAG`. The current header is API v9. On Linux, talk HID directly. `authenticator-rs` (Mozilla, used by Firefox) has full hmac-secret and PRF types. `ctap-hid-fido2` (CTAP 2.0–2.2) supports hmac-secret. `libfido2` via `fido2-rs` is the battle-tested C option. `passkey-rs` (1Password) is a software authenticator, not a hardware-key client.

### Cited Findings — Windows webauthn.dll
- `webauthn.h` current version is `WEBAUTHN_API_VERSION_9`. Version history per the header: v1 baseline incl. hmac-secret (MakeCredential only); v2 credProtect; v3 credBlob and minPinLength; v4 platform credential list/delete; v5 largeBlob; v6 adds the `WEBAUTHN_CTAP_TRANSPORT_HYBRID` transport and bumps options and structs; v7 bumps structs with no comment; v8 adds `pPRFGlobalEval` on MakeCredential; v9 adds `WebAuthNGetAuthenticatorList` (authenticator enumeration). — [microsoft/webauthn webauthn.h](https://github.com/microsoft/webauthn/blob/master/webauthn.h)
- PRF and hmac-secret fields: `MAKE_CREDENTIAL_OPTIONS.bEnablePrf` (v6), `MAKE_CREDENTIAL_OPTIONS.pPRFGlobalEval` (v8, "PRF extension 'eval' values which will be converted into HMAC-SECRET values according to WebAuthn Spec"), `GET_ASSERTION_OPTIONS.pHmacSecretSaltValues` (v6), `ASSERTION.pHmacSecret` (struct v3), and `CREDENTIAL_ATTESTATION.bPrfEnabled` (v5) / `pHmacSecret` (v7). Set `WEBAUTHN_AUTHENTICATOR_HMAC_SECRET_VALUES_FLAG` (0x00100000) in `dwFlags` "if caller wants to provide RAW Hmac-Secret SALT values directly". Raw values must be `WEBAUTHN_CTAP_ONE_HMAC_SECRET_LENGTH` (32 bytes). Otherwise salts are converted as SHA-256("WebAuthn PRF" || 0x00 || value). The header doesn't mention admin rights or other limits for raw salts. One inconsistency: `dwFlags` on MakeCredential is commented "Reserved" while PRF comments refer to it. — [webauthn.h](https://github.com/microsoft/webauthn/blob/master/webauthn.h)
- Direct HID on Windows: "the security key via HID cannot be accessed unless the executing exe has administrator privileges." — [ctap-hid-fido2 README](https://github.com/gebogebogebo/ctap-hid-fido2)
- Corbado's matrix: Windows 10 shows no PRF for platform, security-key or hybrid. Windows 11 before Feb 2026: security keys and hybrid OK, Windows Hello no. Feb 2026 update (KB5077181, builds 26100.7840+ / 26200.7840+): Windows Hello returns PRF. PRF-on-create needs `WEBAUTHN_API_VERSION_8` (Chrome/Edge 147+). — [Corbado PRF guide (updated 2026-09-29)](https://www.corbado.com/blog/passkeys-prf-webauthn)

### Cited Findings — Rust crates (versions from crates.io, 2026-10-31)
- `authenticator` (Mozilla authenticator-rs) 0.5.0 (2025-10-22, about 2.36M downloads, repo pushed 2026-10-15). It supports FreeBSD, Linux, macOS, NetBSD, OpenBSD and Windows over USB HID, with CTAP1, 2.0, 2.1-PRE and 2.1. It is "primarily intended for use by Firefox on platforms that do not provide WebAuthn APIs", and its maturity badge says "release". The source has `HMACGetSecretInput/Output`, `AuthenticationExtensionsPRFInputs/Values/Outputs`, `HmacCreateSecretOrPrf` (make_credentials) and `HmacGetSecretOrPrf` / `HmacSecretExtension` (get_assertion). — [authenticator-rs ctap2-2021 README & src](https://github.com/mozilla/authenticator-rs/tree/ctap2-2021); [crates.io](https://crates.io/crates/authenticator)
- `ctap-hid-fido2` 3.6.0 (2026-09-05, about 377K downloads, single maintainer "gebo", 61 stars). It implements CTAP 2.0/2.1/2.2 over HID, with hmac-secret in make_credential and get_assertion params. It supports macOS and Windows (admin needed), and Linux needs libusb/libudev dev packages. Crypto backend is ring by default, with aws-lc-rs and FIPS options. — [ctap-hid-fido2 README](https://github.com/gebogebogebo/ctap-hid-fido2); [crates.io](https://crates.io/crates/ctap-hid-fido2)
- `webauthn-authenticator-rs` (kanidm) 0.6.2 (about 3.67M downloads, active). Features: `usb`, `nfc`, `bluetooth`, `cable` (caBLE/hybrid authenticator client with QR UI), `win10` (webauthn.dll backend). The grep found hmac-secret only as a MakeCredential extension in its win10 backend and in ctap2 make_credential; I found no hmac-secret GetAssertion support. — [webauthn-rs repo, Cargo.toml & src](https://github.com/kanidm/webauthn-rs/tree/master/webauthn-authenticator-rs)
- `passkey-rs` (1Password): `passkey-client` and `passkey-authenticator` 0.6.0 (2026-10-01) implement WebAuthn L3 and CTAP2 plus a software authenticator. The client "always reports its Authenticator Attachment Modality as 'Platform'". It's for building a passkey provider, not for driving a YubiKey. — [passkey-rs README](https://github.com/1Password/passkey-rs)
- libfido2 bindings: `fido2-rs` 0.6.0 / `libfido2-sys` 0.5.1 (tyan-boot/fido-rs, 2 stars, about 6.6K downloads), thin bindings to Yubico's libfido2. Yubico's deep dive shows the libfido2 call `fido_assert_hmac_secret_ptr`. — [crates.io fido2-rs](https://crates.io/crates/fido2-rs); [Yubico deep dive](https://developers.yubico.com/WebAuthn/Concepts/PRF_Extension/CTAP2_HMAC_Secret_Deep_Dive.html)
- Other small crates: `ctap-fido2` 0.1.3 (HID client with hmac-secret, tested on YubiKey 5C, no PIN protocol v2) and `ctap_hmac` 0.4.5 (2021, stale). — [docs.rs ctap-fido2](https://docs.rs/crate/ctap-fido2/latest); [ctap_hmac](https://rust-digger.code-maven.com/crates/ctap_hmac)

### Inferences
- Windows path: call `webauthn.dll` through the `windows` crate (`Win32_Networking_WindowsWebServices`, as webauthn-authenticator-rs does). Check `WebAuthNGetApiVersionNumber() >= 4`. GET_ASSERTION_OPTIONS v6 with `pHmacSecretSaltValues` has been around since roughly API v3/v4 (Windows 10 21H2 / Windows 11), and the 32-byte raw-salt flag gives byte-identical outputs to libfido2 on Linux. That makes one credential usable on both OSes, as long as the same rpId string is used. The rpId for a native app is just a string the caller passes (for example "keyhandoff.app" or "keyhandoff:vault"). Windows doesn't verify origin for desktop callers, but confirm this. Corbado's "Windows 10: no PRF" is about browser support, and a native app on Windows 10 might still get hmac-secret via webauthn.dll. It needs a test.
- Linux path: `authenticator-rs` is the most mature pure-Rust choice: Firefox's production stack, PRF types, PIN protocol support and multi-OS. `libfido2` (via `fido2-rs` or a small custom FFI) is the most battle-tested (systemd, OpenSSH) but adds a C dependency and the binding crate is tiny. Linux needs a udev rule or `uaccess` tag for `/dev/hidraw*`; most distros ship this for FIDO keys via systemd or libfido2 packages, but verify.
- macOS later: either direct HID (authenticator-rs works, Secretive-style apps do this) or `ASAuthorizationSecurityKeyPublicKeyCredentialProvider`. But on macOS the Secure Enclave covers almost everyone anyway.

### Gaps
- An exact mapping from webauthn.dll API version to Windows build (which build first shipped `pHmacSecretSaltValues` / v6 assertion options) wasn't found. The header has no dates.
- Not verified: whether webauthn.dll on Windows 10 22H2 returns hmac-secret for a security key to a native caller.

---

## B3. Passkeys / platform authenticators and phones via PRF (Windows Hello, iCloud Keychain, GPM, hybrid)

### Takeaway
By late 2026 PRF works on all three big platform authenticators. Apple Passwords/iCloud Keychain has had it since iOS 18 / macOS 15, and Google Password Manager passkeys all have PRF. Windows Hello is new: since the Feb 2026 cumulative update, Windows 11 24H2/25H2 only. Hybrid (phone via QR + BLE) carries PRF. A native Windows 11 app gets hybrid for free through webauthn.dll (transport flag in API v6). On Linux there is no OS WebAuthn stack, so hybrid needs a caBLE client such as kanidm's `cable` feature, which seems to lack hmac-secret on assertion.

### Cited Findings
- Windows Hello PRF: Windows 11 24H2/25H2 with the Feb 2026 update (KB5077181), builds 26100.7840+ / 26200.7840+. Firefox 148+ full, Chrome/Edge 146 get-only, 147+ create and get (needs API v8). Windows 10: no PRF anywhere. — [Corbado PRF guide](https://www.corbado.com/blog/passkeys-prf-webauthn)
- Community reports conflict: a Bitwarden user on build 26200.7840 said PRF worked on some test sites, but Bitwarden still said "encryption is not supported" for a new Windows Hello passkey. Earlier posts said Windows Hello isn't PRF-capable. No Microsoft announcement was found. — [Bitwarden community: PRF via Windows Hello](https://community.bitwarden.com/t/encryption-prf-via-windows-hello-passkey/94236)
- Apple: macOS 15+ (Safari 18+, Chrome 132+, Firefox 139) has platform PRF via iCloud Keychain. iOS/iPadOS 18+ too, but iOS 18.0–18.3 as a hybrid source had data-loss bugs, fixed in 18.4. Security-key PRF in Safari arrived in 26.4, and two WebKit bugs remain open (encrypted hmac-secret returned on USB/NFC; null PRF on YubiKey Bio / intervalUV keys). — [Corbado PRF guide](https://www.corbado.com/blog/passkeys-prf-webauthn)
- Android: "All passkeys in Google Password Manager include PRF". Chrome, Edge and Samsung Internet support it; Firefox 149+ since March 2026. — [Corbado PRF guide](https://www.corbado.com/blog/passkeys-prf-webauthn)
- Provider matrix (Aug 2026 snapshot): PRF on create and get for Apple Passwords, GPM, Windows Hello (with the updates above), 1Password, Proton Pass, KeePassDX, Enpass and Keeper. Microsoft Password Manager (Edge) creates only, and get fails. Bitwarden is inconsistent across platforms. Dashlane and NordPass: no. Samsung Pass: get only. The Chrome profile authenticator: no PRF. "A capability flag is not a guarantee." — [Corbado PRF guide](https://www.corbado.com/blog/passkeys-prf-webauthn)
- Hybrid: Corbado shows hybrid PRF as working on Windows 11 (even before Feb 2026), macOS 15+, iOS 18.4+ and Android. It has no separate hybrid test data. — [Corbado PRF guide](https://www.corbado.com/blog/passkeys-prf-webauthn)
- webauthn.dll API v6 adds `WEBAUTHN_CTAP_TRANSPORT_HYBRID` (0x20). The old `pLinkedDevice` / hybrid storage linked-data structures are marked deprecated. — [webauthn.h](https://github.com/microsoft/webauthn/blob/master/webauthn.h)
- `webauthn-authenticator-rs` has a `cable` feature ("caBLE / hybrid authenticator"), QR-code UI support, a `bluetooth` feature (btleplug, with bluez notes for Linux) and tunnel-server override options. — [webauthn-authenticator-rs Cargo.toml](https://github.com/kanidm/webauthn-rs/blob/master/webauthn-authenticator-rs/Cargo.toml)
- Enpass added PRF for passkey create and sign-in on Windows (release 2026-02-23). — [Enpass Windows release notes](https://www.enpass.io/release-notes/windowspc/)

### Inferences
- Coverage strategy for "basically every computer":
  - Windows 11 (any TPM state): webauthn.dll gives three PRF sources through one API and one OS dialog. Windows Hello on Feb-2026+ builds is backed by the TPM or software, so it is not "outside the TPM". The other two are USB/NFC security keys and phones via hybrid. Hybrid needs Bluetooth on the PC, which many desktops lack. Without Bluetooth, the fallback is a USB security key.
  - Linux: security keys via HID are solid. Phone-via-hybrid would need a custom caBLE client plus BLE plus hmac-secret on assertion, which kanidm doesn't seem to have. Treat that as R&D, not a near-term option.
  - macOS: the Secure Enclave covers it, and phone or iCloud passkeys are possible via ASAuthorization (not researched in detail).
- Phone-as-authenticator gives near-universal coverage on Windows 11 (nearly everyone has a phone with GPM or iCloud passkeys and PRF). Caveat: the derived key lives in a synced passkey. Anyone who takes over the user's Google or Apple account and syncs the passkey to a new device can derive it. That is a different trust model from a device-bound hardware key, though still well above "malware can read it".
- Because the phone's passkey syncs and is backed up, losing the phone usually doesn't lose the wrap, unlike a single YubiKey. This is a real recovery advantage.

### Gaps
- Whether a native (non-browser) caller on Windows 11 can request PRF from a hybrid phone through webauthn.dll with a non-web rpId: the phone sees the rpId, and GPM or iCloud may refuse or label oddly an rpId that isn't a domain with a valid .well-known association. Not verified. This is the key open question for "phone as universal fallback".
- No primary Microsoft documentation of Windows Hello PRF was found, only Corbado and community reports.

---

## B4. How password managers and tools use these

### Takeaway
Bitwarden's PRF passkey unlock is browser-extension or web only. systemd-cryptenroll is the reference for native hmac-secret key wrapping, with PIN and UP on by default and recovery keys recommended. KeePassXC's YubiKey support uses the older HMAC-SHA1 challenge-response (OTP applet), and its PRF support isn't in a release. age-plugin-yubikey (PIV) and 1Password weren't verified this session.

### Cited Findings
- Bitwarden: login and unlock with passkeys "currently supported in the browser extension in Chromium-based browsers and the web app". It requires a PRF-capable browser and authenticator, and there is a known Linux defect with the extension. A third-party guide claims desktop-app support, which conflicts with this. — [Bitwarden Help: Log in & Unlock with Passkeys](https://bitwarden.com/help/login-with-passkeys.md); [tech-insider (unverified claim)](https://tech-insider.org/bitwarden-passwordless-login-passkeys-setup-2026/); [Bitwarden community announcement](https://community.bitwarden.com/t/you-can-now-unlock-your-vault-with-a-passkey/93556)
- systemd-cryptenroll `--fido2-device=` (hmac-secret): `--fido2-with-client-pin` defaults to yes, `--fido2-with-user-presence` to yes and `--fido2-with-user-verification` to no. `--fido2-credential-algorithm` (es256 default, rs256, eddsa) was added in v251, and `--fido2-salt-file` in v257. Parameters (credential ID, salt) are stored in a LUKS2 header token by default. Recovery keys are meant as the fallback "if the token is lost". With several UV tokens enrolled it tries each one, which means repeated prompts. — [systemd-cryptenroll(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-cryptenroll.html)
- KeePassXC: "No released version supports PRF". Positive PRF results probably came from dev builds. — [Corbado PRF guide](https://www.corbado.com/blog/passkeys-prf-webauthn)
- Dashlane uses PRF for its own vault but returns nothing to third-party sites. 1Password works as a PRF-capable passkey provider. — [Corbado PRF guide](https://www.corbado.com/blog/passkeys-prf-webauthn)

### Inferences
- The systemd-cryptenroll defaults (PIN plus UP, random salt and credential ID stored next to the ciphertext, a recovery key alongside) are a good template for KeyHandoff's external-authenticator wrap.

### Gaps
- KeePassXC's YubiKey HMAC-SHA1 challenge-response (OTP slot 2) details. Background knowledge only, unverified: it works through the OTP/HID interface and needs no admin on Windows, but it's YubiKey or OnlyKey specific, and the shared HMAC secret can be programmed into several keys for backup.
- age-plugin-yubikey (PIV, P-256 ECDH, PIN and touch policies) was not fetched.
- 1Password's own use of PRF or passkeys for account unlock was not verified.

---

## B5. Risks: lost keys, presence-only vs PIN, malware piggy-backing on a touch

### Takeaway
(1) Losing the only authenticator loses the wrap. Always enroll a second authenticator or a recovery code, as Yubico and systemd both recommend. (2) Touch-only (UP) proves a human touched the key, not which request they approved. A PIN (UV) adds a knowledge factor, and hmac-secret gives different outputs with and without UV, so pick the mode at enrollment. (3) Same-user malware can read the stored credential ID and salt and send its own GetAssertion. If the user touches the blinking key thinking it is KeyHandoff's request, malware gets the secret. This is inherent to UP-only FIDO on a compromised host; a UV PIN only helps if the PIN is entered somewhere malware can't read.

### Cited Findings
- Data encrypted under a key-derived secret is unrecoverable if the key is lost, so use multi-device envelope encryption. — [Yubico deep dive](https://developers.yubico.com/WebAuthn/Concepts/PRF_Extension/CTAP2_HMAC_Secret_Deep_Dive.html)
- Recovery keys exist as a fallback for a lost hardware token. Keep a non-PKCS#11 method enrolled for rollover. — [systemd-cryptenroll(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-cryptenroll.html)
- Once the client has the secret, the authenticator is "not directly involved"; the host app must protect it (KDF right away, wipe). — [Yubico deep dive](https://developers.yubico.com/WebAuthn/Concepts/PRF_Extension/CTAP2_HMAC_Secret_Deep_Dive.html)
- Native callers bypass the browser's "WebAuthn PRF" salt prefix and the origin binding, so domain separation is the app's job. — [Yubico deep dive](https://developers.yubico.com/WebAuthn/Concepts/PRF_Extension/CTAP2_HMAC_Secret_Deep_Dive.html)
- Secure Enclave keys can't be exported or backed up: a new Mac means new keys. — [Secretive README](https://github.com/maxgoedjen/secretive)
- Synced passkeys: "Lost passkey = lost data", so plan backup and recovery before shipping PRF-based encryption. — [Corbado PRF guide](https://www.corbado.com/blog/passkeys-prf-webauthn)

### Inferences
- Malware piggy-backing: on Linux, any same-user process can open `/dev/hidraw*` (uaccess) and race KeyHandoff. On Windows, any same-user process can call webauthn.dll with KeyHandoff's rpId, credential ID and salt, all readable from disk. The OS dialog may name the rpId but not the calling app (unverified). Mitigations:
  - (a) Require UV. On Windows the PIN goes into the OS-owned webauthn dialog, which malware can't trivially keylog. On Linux the app collects the PIN itself, so a keylogger defeats UV. A biometric key (YubiKey Bio) fixes that.
  - (b) Only ask for a touch when the user explicitly starts an unlock, and show "touch now" in KeyHandoff. An unexpected blink is then a visible anomaly.
  - (c) Unwrap once per session, so touches are rare and expected.
  - (d) Accept that FIDO wrapping raises the bar from "malware reads a file" to "malware must win a race for a human touch, and it gets the master key once". Malware that can do that can usually also read KeyHandoff's process memory after a legitimate unlock. So the real gain is against offline theft, backups and passive file-grabbing malware, not against an active same-user attacker. The same limit applies to TPM and Secure Enclave wraps on Windows and Linux. macOS is the exception, because keychain access groups stop other-team processes from using the SE key at all (A3).
- macOS SE plus `.userPresence` doesn't have the piggy-back problem in the same form: only KeyHandoff-signed code can start the decrypt, and the Touch ID sheet shows the app's reason string.
- Recommended enrollment model: wrap the master key separately under each enrolled method (SE or TPM key, FIDO key #1, FIDO key #2 or phone passkey, offline recovery code), so any one can unwrap. Re-wrapping needs the plaintext master key, so enrolling a new method needs one unlock first, which matches systemd's model.

### Gaps
- Whether the Windows webauthn.dll dialog shows the calling process or only the rpId. Not verified; it matters for how much the piggy-back risk can be explained to users.
- I found no published analysis of malware racing a legitimate hmac-secret request on desktop. The risk analysis above is my own reasoning.
