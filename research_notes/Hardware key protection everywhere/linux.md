# Linux: hardware-backed protection of a local master key, gated by a user prompt, judged against same-user malware

Scope: KeyHandoff (Rust + Tauri) holds a 32-byte vault master key. On Linux today it lives in the Secret Service (GNOME Keyring / KWallet) or a 0600 file, and both hand it to any process running as the same user with no prompt. These notes survey what Linux offers to (a) back that key with a TPM 2.0 behind a PIN or passphrase, (b) fall back when there is no TPM, and (c) what any of it is worth when malware already runs as the same user.

Labeling convention. Under "Cited Findings" each line is backed by a fetched page. Under "Inferences", lines marked [bg] come from the researcher's background knowledge and were not re-verified this session. The report writer should treat [bg] lines as plausible but unconfirmed.

Local observation from the research machine: Pop!_OS 22.04 on kernel 7.1.1 has no /dev/tpm* nodes (no TPM exposed, or it's disabled in firmware). The user `juno` is not in a `tss` group, and /proc/sys/kernel/yama/ptrace_scope = 1. So "no TPM" is a real case on the developer's own laptop.

---

## 1. TPM 2.0 from userspace in Rust: crates, C libraries, the resource manager, device permissions

### Takeaway
`tss-esapi` 7.7.0 (Oct 2026) is the mature Rust path. It wraps the tpm2-tss C ESAPI, so it needs libtss2-esys, tctildr and mu ≥ 2.3.3 found via pkg-config at build time, and the shared libraries at run time. Talk to the kernel resource manager at /dev/tpmrm0, not to tpm2-abrmd. The real blocker on a desktop: upstream udev rules give /dev/tpmrm0 to the `tss` group, so a normal desktop user usually can't open it without being added to that group (or a distro or app adding a `uaccess` rule).

### Cited Findings
- crates.io lists `tss-esapi` 7.7.0 and `tss-esapi-sys` 0.6.0, both updated 2026-10-09 ("Rust-native wrapper around TSS 2.0 Enhanced System API"). — [crates.io API: tss-esapi](https://crates.io/api/v1/crates/tss-esapi), [tss-esapi-sys](https://crates.io/api/v1/crates/tss-esapi-sys)
- tss-esapi-sys needs tpm2-tss ≥ 2.3.3 (`tss2-esys`, `tss2-tctildr`, `tss2-mu`), found with pkg-config. By default it uses pre-committed bindings per target triple, with an optional `generate-bindings` feature. The page lists only `x86_64-unknown-linux-gnu` among committed targets, and cross-compiling needs TSS libraries built for the target. The page does not mention a `bundled` or static feature. — [docs.rs tss-esapi-sys](https://docs.rs/crate/tss-esapi-sys/latest)
- The rust-tss-esapi repo (Parallax Secure) offers raw FFI (tss-esapi-sys) plus "abstracted versions" for convenience. CI tests the latest stable Rust and Rust 1.85.0 on Ubuntu, plus Fedora 38 and rawhide compilers. The repo has more than 1,000 commits and a security policy. — [GitHub parallaxsecond/rust-tss-esapi](https://github.com/parallaxsecond/rust-tss-esapi)
- Upstream tpm2-tss udev rules: the raw /dev/tpm* device is limited to the `tss` user, and "tss group members can access tpmrm devices". An earlier tpm2-abrmd issue noted that the old rule covered only /dev/tpm[0-9]* and missed /dev/tpmrm[0-9]*, and the fix added group access for tpmrm. — [tpm2-tss dist/tpm-udev.rules](https://github.com/tpm2-software/tpm2-tss/blob/ede63dd1ac1f0a46029d457304edcac2162bfab8/dist/tpm-udev.rules); [tpm2-abrmd issue #334](https://github.com/tpm2-software/tpm2-abrmd/issues/334)
- The usual guidance is to add the user to `tss` (`usermod -aG tss`). One project documents this for Ubuntu and Arch. No source found shows a stock `TAG+="uaccess"` rule for tpmrm0. — [search summary of NixOS tpm2 module / third-party docs](https://github.com/NixOS/nixpkgs/blob/release-24.05/nixos/modules/security/tpm2.nix); [psanford/tpm-fido](https://github.com/psanford/tpm-fido)
- systemd-creds accepts `--tpm2-device=` with a device node "such as /dev/tpmrm0" or `auto`, which confirms tpmrm0 as the normal target for modern tooling. — [systemd-creds(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-creds.html)
- ssh-tpm-agent (Go) offers "TPM session encryption" and an optional owner password, and has a `--swtpm` mode that it warns "provides no security properties and should only be used for testing". — [Foxboron/ssh-tpm-agent](https://github.com/Foxboron/ssh-tpm-agent)

### Inferences
- [bg] The kernel resource manager /dev/tpmrm0 (since Linux 4.12) virtualizes object and session handles per open file descriptor. That makes tpm2-abrmd (a D-Bus daemon) unnecessary for a single app. Use the `device:/dev/tpmrm0` TCTI with tss-esapi (`TctiNameConf::Device`) and skip the abrmd dependency.
- [bg] The `tss` group grants TPM access to every process of that user, malware included. So group membership is a precondition, not a defense. The TPM's protection must come from the authValue (PIN), not from who can open the device.
- [bg] Distro packaging: Ubuntu, Fedora and Arch install the tpm2-tss udev rule with the `tss` group when the tpm2-tss package is present (Fedora and Ubuntu usually have it, since systemd-cryptenroll and clevis pull it in). A desktop user is not in `tss` by default on any of them, including Pop!_OS. Not verified per distro this session. The Arch Wiki returned an Anubis bot block.
- Practical install path for KeyHandoff: ship a small udev rule (`KERNEL=="tpmrm[0-9]*", TAG+="uaccess"` or `GROUP="tss"`) in the .deb or .rpm, or ask the user to run `usermod -aG tss`. Either needs root once. Detect "TPM present but EACCES" and explain it in the UI.
- Runtime linking: tss-esapi needs libtss2-esys.so etc. at run time. That's fine for .deb or .rpm (declare a dependency). An AppImage must bundle them, and a Flatpak must build them into the bundle (see section 6). Alternative: open tss-esapi as an optional runtime path behind a feature flag, so the app still starts without libtss2.
- Pure-Rust alternatives to tss-esapi (no C dependency) exist but are less mature; none was evaluated this session. Go tooling (go-tpm, which Foxboron's tools use) is the mature non-C stack, which shows a pure implementation is feasible.

### Gaps
- No primary source confirming default `tss` membership or a uaccess rule per distro (Ubuntu 24.04/26.04, Fedora 44/45, Arch, Pop!_OS 24.04 COSMIC). The Arch Wiki TPM page was blocked by Anubis.
- No current information on whether tss-esapi supports static or vendored libtss2.
- tpm2-pkcs11 was not researched in depth. [bg] It exposes TPM keys via PKCS#11 with a SQLite store and per-token PIN. For a single symmetric master key it adds complexity without clear benefit over direct ESAPI sealing.

---

## 2. Sealing a key with an authValue (PIN); dictionary-attack lockout; PCR binding

### Takeaway
Seal the 32-byte key as a TPM keyedhash data object under the storage root key, with an authValue derived from the user's PIN (or with PolicyAuthValue in the policy). Every wrong PIN increments the TPM's global dictionary-attack counter. Typical Linux-unconfigured TPMs have vendor defaults, and Windows configures 32 tries with one forgiven every 10 minutes. That rate limit is what makes a short PIN tolerable, but only against offline or bulk guessing. Same-user malware can wait for the user to type the PIN, or burn the DA counter (a denial of service). Don't bind to PCRs 0, 2 or 4 for a desktop app: firmware, boot-loader and kernel updates break them. Either no PCRs (systemd's default) or at most PCR 7 / a signed policy.

### Cited Findings
- systemd-cryptenroll `--tpm2-with-pin`: by default "the PIN is hardened with Argon2id before it is passed to the TPM", which makes "the TPM a second factor rather than a single point of failure". Argon2id memory is auto-sized from RAM (minimum 64 MiB) with a 2 s target. A `direct` mode "hashed with PBKDF2 and used directly" is compatible with v253+. Any characters are allowed, not just digits. — [systemd-cryptenroll(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-cryptenroll.html)
- systemd-cryptenroll warns that an "incorrect PIN entry when unlocking increments the TPM dictionary attack lockout mechanism", which "may lock out users for a prolonged time, depending on its configuration". "The lockout mechanism is a global property of the TPM", and systemd "does not control or configure the lockout mechanism". Inspect or change it with `tpm2_getcap` / `tpm2_dictionarylockout`. — [systemd-cryptenroll(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-cryptenroll.html)
- `tpm2_dictionarylockout` sets `-n` max tries, `-t` recovery time for DA-protected objects, and `-l` lockout-handle recovery. `-c` clears the lockout with the lockout-hierarchy auth. The man page example (`-n 5 -t 6 -l 7`) is illustrative, not a default. — [tpm2-tools tpm2_dictionarylockout(1)](https://tpm2-tools.readthedocs.io/en/stable/man/tpm2_dictionarylockout.1/)
- TPM 2.0 DA is a global lockout, not per key, and "a single authorization success can't reset the TPM's anti-hammering protection". On Windows, TPM 2.0 is configured "to lock after 32 authorization failures and to forget one authorization failure every 10 minutes", so a full reset takes 320 minutes. Legacy parameters were 32 immediate guesses then one every two hours (about 4,415 per year, so a 4-digit PIN falls in a little over two years). That's why Windows raised the minimum BitLocker PIN to 6 characters in 1703. Keys with no authValue still work while the TPM is locked. — [Microsoft Learn: TPM fundamentals, anti-hammering](https://learn.microsoft.com/en-us/windows/security/hardware-security/tpm/tpm-fundamentals)
- PCR guidance: "it is typically not advisable to use PCRs such as 0 and 2", since they change with firmware and OS updates. Signed policies (`--tpm2-public-key`, default PCR 11) are preferred because "the measurements will change on every update, but signatures should remain unchanged". The default for both cryptenroll and creds is no PCRs. — [systemd-cryptenroll(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-cryptenroll.html); [systemd-creds(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-creds.html)
- systemd v262+ "pin the credential to the TPM2 SRK to defend against MITM interposer attacks" (incompatible with older versions). — [systemd-creds(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-creds.html)

### Inferences
- [bg] Recommended object design: `Create` a `KeyedHash` sealed-data object (`sensitive.data` = the 32-byte key, or better, a random wrapping key that encrypts the vault key on disk) under a primary ECC P-256 SRK that is re-derived from the owner hierarchy's standard template, so nothing persistent is needed. Set `userWithAuth` with authValue = KDF(PIN), or use `PolicyAuthValue` in a policy digest when combining with other policy terms (PolicyPCR, PolicySigned). Don't set `noDA`. Store the TPM2B_PUBLIC and TPM2B_PRIVATE blobs in the app data directory. Unseal with an HMAC or policy session that is salted to the SRK and uses parameter encryption (`TPMA_SESSION_ENCRYPT` on response). Then the unsealed key never crosses the LPC/SPI bus or the /dev/tpmrm0 interface in clear, and the PIN-derived HMAC is bound to a secret the bus sniffer doesn't know.
- [bg] PolicyAuthValue and PolicyPassword: PolicyAuthValue makes the session HMAC prove knowledge of the authValue. PolicyPassword sends the authValue in plaintext in the command (protected only if the session encrypts parameters). Prefer PolicyAuthValue with a salted session. Both count against DA when wrong.
- Following systemd's design, Argon2id-stretch the PIN before using it as the authValue. Then even an attacker who later breaks the TPM (or extracts the blob plus a TPM vulnerability) still faces a memory-hard KDF. Also bind the sealed key with an on-disk salt so the blob alone is useless.
- DA protects a short PIN only against an attacker who must go through this TPM. Against same-user malware it buys rate limiting (bulk guessing is impractical), but:
  - Malware can trigger lockout on purpose (DA is global, so this also hurts BitLocker-like users, LUKS PINs and ssh-tpm-agent). That's a denial of service, not a disclosure.
  - Malware can capture the PIN as it's typed (X11; see section 5) or read the unsealed key from the app's memory after unlock (ptrace or /proc/pid/mem if allowed). The TPM can't help after unseal.
  - The TPM still wins against offline theft: a copied vault file plus a copied blob is useless off-machine, and an exfiltrated blob can't be brute-forced elsewhere.
- [bg] On Linux nobody configures DA parameters by default. TPMs ship with vendor defaults (often around 32 tries and around 2 h recovery on fTPM or PTT, but this varies). The app should read `TPM_PT_MAX_AUTH_FAIL`, `TPM_PT_LOCKOUT_INTERVAL` and `TPM_PT_LOCKOUT_COUNTER` via GetCapability to show "N tries left" and refuse very short PINs if maxTries is large. KeyHandoff shouldn't change the global DA settings (that needs lockout auth, and other tools share them).
- Desktop PCR advice: bind to no PCRs (the user prompt is the gate, as with systemd's default). Binding to PCR 7 (Secure Boot state) is defensible but breaks on SB key or db updates and when SB is toggled. Signed-PCR-11 policies only make sense on UKI-based distros, which few desktops ship. PCRs don't defend against same-user malware anyway: the machine is booted correctly when the malware runs.
- Recovery: TPM clear, motherboard swap or fTPM reset (some firmware updates reset AMD fTPM or Intel PTT) destroys the sealed key. Keep a recovery path (Argon2id passphrase-wrapped copy, or a printed recovery key) exactly as LUKS keeps a passphrase slot.

### Gaps
- No primary source for factory DA defaults on Linux (TCG spec text, or Infineon / STM / Nuvoton / AMD fTPM / Intel PTT values).
- No tss-esapi worked example of PolicyAuthValue sealing was retrieved this session. The tss-esapi docs.rs abstraction (`TransientKeyContext`) was not examined.

---

## 3. How existing tools do it, and what to copy

### Takeaway
systemd-cryptenroll `--tpm2-with-pin` is the best model: TPM-sealed secret, authValue = Argon2id(PIN), no PCRs by default, DA counter as the rate limit, and a separate recovery slot. systemd-creds has no PIN option, so it's TPM-only or host-key encryption (a machine binding, not a user gate), and it's not useful as a prompt. ssh-tpm-agent and age-plugin-tpm (Foxboron, Go) show the per-user desktop pattern: keys sealed to the TPM in user files, optional PIN, TPM session encryption.

### Cited Findings
- systemd-creds `--with-key=` accepts `host`, `tpm2`, `host+tpm2`, `null`, `auto`, `auto-initrd` (since v250). The doc mentions no PIN or passphrase option for TPM2. A TPM2-derived key is "not stored on the host system". Encryption is AES256-GCM with a key that is SHA256 of the secret(s). `--user` / `--uid=` (v256) mixes UID, username and machine-id into the key for per-user-manager credentials. — [systemd-creds(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-creds.html)
- systemd-cryptenroll: Argon2id-hardened PIN as TPM authValue, DA caveats, PCR brittleness, signed policies, pcrlock. — [systemd-cryptenroll(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-cryptenroll.html)
- ssh-tpm-agent: keys are generated in the TPM and stored "sealed" in `.tpm` files in the TPM 2.0 Key Files format (via go-tpm-keyfiles), and are bound to that hardware. `ssh-tpm-keygen` prompts for a passphrase (empty allowed). PIN support allows "lower-entropy PINs" backed by TPM DA protection. It also offers TPM session encryption and remote wrapping to a client's SRK. — [Foxboron/ssh-tpm-agent](https://github.com/Foxboron/ssh-tpm-agent)
- age-plugin-tpm: identities are "created on the TPM, sealed outside of it". `--generate --pin` requires a PIN for use, and it offers TPM session encryption. It warns that the `AGE_TPM_PIN` env var is insecure because it "can be read from /proc/$PID/environ". — [Foxboron/age-plugin-tpm](https://github.com/Foxboron/age-plugin-tpm)
- oo7 (Rust Secret Service: client library, daemon `oo7-daemon`, portal backend `oo7-portal`, PAM integration) is replacing gnome-keyring. The Arch Wiki says that from GNOME 51 oo7 provides default key storage, and Fedora ELN ships oo7-daemon 0.7.0~beta, which obsoletes gnome-keyring < 51. A 2024 All Systems Go talk described TPM-backed credentials for oo7-daemon via systemd per-user credentials. The oo7 README itself mentions no TPM or systemd-creds support. — [linux-credentials/oo7](https://github.com/linux-credentials/oo7); [ASG 2024 talk](https://cfp.all-systems-go.io/all-systems-go-2024/talk/8TMT9T/); [Arch Wiki GNOME/Keyring via search](https://wiki.archlinux.org/title/GNOME/Keyring); [fosslinux on Fedora 45 oo7](https://www.fosslinux.com/162689/fedora-oo7-secrets-service-explained.htm)

### Inferences
- What to copy for KeyHandoff:
  1. cryptenroll's PIN pipeline (Argon2id, then TPM authValue; salt stored beside the blob).
  2. ssh-tpm-agent's on-disk TPM 2.0 Key Files format (an interoperable PEM-ish `TSS2 PRIVATE KEY` ASN.1 blob) for the sealed object.
  3. Salted sessions with parameter encryption (both Foxboron tools and systemd ≥ 262 SRK pinning) against bus interposers.
  4. No PCRs by default.
  5. A recovery passphrase slot.
  6. Never pass the PIN via env vars or argv (the age-plugin-tpm warning).
- [bg] clevis (tpm2 pin) seals to PCRs without a user PIN; it's built for unattended unlock. tpm2-totp proves boot integrity to the user (a TOTP shown at boot); it doesn't protect a user secret. keylime is remote attestation for fleets. None of these fit a per-user, prompt-gated desktop secret. They aren't worth copying beyond the sealing primitive.
- systemd-creds `--user` (v256+) could serve as a "machine-binding" layer (TPM + host, no prompt). It doesn't stop same-user malware: any process of that user can run `systemd-creds decrypt --user`, which goes through systemd's service. [bg] Verify whether per-user decryption is brokered by the io.systemd.Credentials Varlink service, and whether that service authenticates the caller beyond UID.
- If oo7 lands TPM-sealed keyrings, it protects data at rest. The Secret Service D-Bus API still serves any same-user client once unlocked (section 5), so it doesn't fix KeyHandoff's actual problem.

### Gaps
- Did not verify clevis, tpm2-totp or keylime docs this session (the descriptions are background knowledge).
- Did not confirm whether systemd 258–262 added a PIN option to systemd-creds (the current man page shows none).

---

## 4. Presence prompts: fprintd, polkit, PAM. Cryptographic gate or yes/no?

### Takeaway
On Linux all three are yes/no decisions made by a privileged service. They release no key material. fprintd only reports "matched / didn't match". A same-user process can't forge a polkit authorization for someone else's action, but nothing forces it to ask KeyHandoff's polkit action before reading KeyHandoff's secret, because the secret isn't bound to that decision. So a fingerprint can't release a key on Linux today (confirmed). The only cryptographic gates are a TPM authValue (PIN) or an external authenticator (FIDO2 hmac-secret with UV or UP).

### Cited Findings
- Matthew Garrett (Dec 2023): with fprint, in either reader design the OS gets only "The fingerprint matched" or "The fingerprint didn't match". "We can't easily do this under Linux because we don't have an interface to store those secrets." "We can't use the TPM because there's no secure communications channel between the fingerprint reader and the TPM." Conclusion: fingerprint unlock doesn't unlock the keyring "because there's currently no secure way to do that". Contrast Apple, where the enclave releases material only on a match. — [mjg59: Why does Gnome fingerprint unlock not unlock the keyring?](https://mjg59.dreamwidth.org/68537.html)
- The Fedora wiki notes gnome-keyring can't unlock from a fingerprint login and asks for the password, since the keyring is encrypted with the login password. A fprint PPA maintainer says a fingerprint match can't be a source of secret data. — [Fedora Features/Fingerprint](https://fedoraproject.org/wiki/Features/Fingerprint); [noratek guide](https://noratek.dev/howto/gnome-keyring-fingerprint-login-prompt/); [HN discussion](https://news.ycombinator.com/item?id=38527876)

### Inferences
- [bg] polkit: a polkit check (`CheckAuthorization` over the system bus) is enforced by the mechanism (a privileged daemon) that asks. It protects privileged operations from unprivileged callers. If KeyHandoff (unprivileged) itself calls polkit and then reads a key it already can read, malware just reads the key file or Secret Service entry directly. A polkit gate only means something if a separate privileged helper (a root or dedicated-UID daemon) holds the key or holds sole TPM access and releases it only after `auth_self` succeeds. That's a real design option (a "keyhandoff-keyd" system service with its own UID owning /dev/tpmrm0 and the sealed blob), but it raises the install bar. A same-user attacker can then still (a) ask the helper and race the user's approval, (b) on X11 click the dialog, or (c) read the key from KeyHandoff's memory after release.
- [bg] pkexec is set-uid launch of a program, not a key-release mechanism; unsuitable here.
- [bg] PAM: an unprivileged app can only verify the user's own password via the setuid `unix_chkpwd` helper (pam_unix). That's still a yes/no and yields no key. pam_fprintd likewise. The one useful PAM pattern is pam_gnome_keyring / oo7 PAM, which take the typed password at login and use it as key material. That works for a password, not for a fingerprint.
- fprintd itself runs as root on the system bus. [bg] Its D-Bus API (net.reactivated.Fprint) lets the session user's processes claim the device and run Verify. So malware can also trigger a verify, and the user may then touch the sensor thinking it's KeyHandoff asking (prompt confusion).
- Net: for a cryptographic gate, the TPM PIN is the only built-in option. A fingerprint can at most be a UX convenience layered on a session cache (for example, unlock with PIN once, then use fingerprint as a yes/no to re-arm after idle timeout while the key stays in memory). Present that honestly as "convenience, not protection".

### Gaps
- No source found on match-on-chip readers on Linux with secure-channel (SDCP-like) support that could bind to a TPM. Windows has SDCP; Linux has no equivalent per mjg59 (2023). No 2026 update found.
- GNOME/systemd TPM-backed keyring work was mentioned in comments only; no shipped status confirmed.

---

## 5. Same-user attack surface on Linux and in-process hardening

### Takeaway
On a typical desktop, same-user malware can read the Secret Service, the 0600 file, environment variables and (where ptrace_scope = 0, as on Fedora) the app's memory via ptrace or /proc/pid/mem. Yama ptrace_scope = 1 (Ubuntu, Pop!_OS, Arch; Fedora still 0) blocks non-descendant attach and, by the same check, /proc/pid/mem. `prctl(PR_SET_DUMPABLE, 0)` further blocks same-UID ptrace and /proc/pid/mem even at scope 0. memfd_secret hides pages from the kernel's direct map and from GUP-based access, but is off by default on many kernels. None of this stops LD_PRELOAD at launch, replacing the binary or .desktop file in the user's home, or X11 keylogging and injection. Wayland closes the input side.

### Cited Findings
- Yama ptrace_scope: 0 = any same-UID process may PTRACE_ATTACH if the target is dumpable (not dumpable if it called `prctl(PR_SET_DUMPABLE...)`, among other cases). 1 = only descendants, or a PID named via `PR_SET_PTRACER`. 2 = CAP_SYS_PTRACE only. 3 = no attach, irreversible. Rationale: a compromised app (the Pidgin example) attaching to Firefox, SSH or gpg-agent to steal credentials. — [kernel.org Yama docs](https://docs.kernel.org/admin-guide/LSM/Yama.html)
- /proc/pid/mem access "is governed by a ptrace access mode PTRACE_MODE_ATTACH_FSCREDS check". — [proc_pid_mem(5)](https://man7.org/linux/man-pages/man5/proc_pid_mem.5.html)
- Fedora: `elfutils-default-yama-scope`, pulled in by elfutils-libs, sets ptrace_scope = 0. The change to restore the kernel default 1 is marked a "rejected Change proposal" (page updated 2026-01-21). A later discussion and a news item claim Fedora 45 changes this; unverified. Ubuntu and Arch are reported at 1 (secondhand). The research machine (Pop!_OS 22.04) reads 1 locally. — [Fedora Change page](https://fedoraproject.org/wiki/Changes/Restrict_ptrace_for_unprivileged_users_to_child_processes_to_match_kernel_default); [Fedora discussion](https://discussion.fedoraproject.org/t/increase-security-by-adjusting-kernel-settings-by-default-kernel-yama-ptrace-scope-net-core-bpf-jit-harden-both-defaulting-to-1/161498); [newer Fedora discussion](https://discussion.fedoraproject.org/t/new-proposal-about-kernel-yama-ptrace-scope-two-perspectives-on-this-case-im-open-to-suggestions/172815)
- memfd_secret (Linux 5.14): memory only the fd holder maps, with pages removed from the kernel page tables, though "no 100% guarantee". It's locked like mlock and counted against RLIMIT_MEMLOCK, and hibernation is blocked while it exists. Before 6.5 it was disabled by default and needed `secretmem.enable=y`; ENOSYS if unavailable. There's no glibc wrapper (use `syscall`). The page lists ptrace via a privileged process as a remaining exfiltration route. — [memfd_secret(2)](https://man7.org/linux/man-pages/man2/memfd_secret.2.html)
- age-plugin-tpm: env vars are readable from /proc/$PID/environ. — [Foxboron/age-plugin-tpm](https://github.com/Foxboron/age-plugin-tpm)

### Inferences
- [bg] Because Yama's hook covers PTRACE_MODE_ATTACH checks, scope 1 also blocks a non-ancestor same-user process from opening /proc/<pid>/mem for read. But a malicious *parent* (malware that launches KeyHandoff, or a hijacked .desktop / autostart entry) is an ancestor and passes.
- [bg] `prctl(PR_SET_DUMPABLE, 0)`: makes /proc/<pid>/ files root-owned and makes same-UID ptrace and /proc/pid/mem fail (access needs CAP_SYS_PTRACE), at any ptrace_scope. It also disables core dumps. Caveats: the flag is inherited across fork but reset by execve, so set it early in the running core process itself. Tauri's WebKitGTK web processes are separate children (WebKitWebProcess) and aren't covered unless they also set it. Keep the master key only in the Rust core process, never in JS or WebView memory.
- [bg] mlock / `MADV_DONTDUMP`: keep the key out of swap and core dumps; it doesn't defend against a live reader. memfd_secret: the kernel refuses GUP on secretmem VMAs, so /proc/pid/mem and PTRACE_PEEKDATA can't read those pages even for a permitted tracer. But a tracer can still inject code or redirect execution in the process to copy the secret out. So it's a speed bump, not a boundary. Treat it as optional (ENOSYS fallback to mlock'd memory). Many distro kernels now enable it, but this is unverified; check `secretmem.enable` in the target distros' kernel configs.
- [bg] LD_PRELOAD / LD_LIBRARY_PATH: same-user malware can edit ~/.profile, ~/.config/environment.d, ~/.config/autostart or ~/.local/share/applications/*.desktop to launch KeyHandoff with an injected library. Nothing inside the app fully prevents this. Mitigations are partial: check `getauxval(AT_SECURE)` (not set for non-setuid), scan /proc/self/maps for unexpected libraries, and ignore env in the launcher. Docs should state this plainly.
- [bg] X11: any X client can read all keystrokes (XRecord, XInput2 raw events, XQueryKeymap) and inject input (XTest), read the clipboard, and screenshot. So a PIN typed into KeyHandoff on X11 is capturable, and "click OK" prompts are clickable by malware. Wayland compositors isolate input, so clients can't read other clients' keystrokes or synthesize input except via portals (RemoteDesktop/InputCapture) that prompt. This connects to the existing HAND-39 clipboard tripwire work. Detect `XDG_SESSION_TYPE=x11` and warn that the PIN gate is weaker there.
- [bg] Secret Service / D-Bus: any process on the user's session bus can call org.freedesktop.secrets (gnome-keyring, KWallet, oo7) and read any item once the collection is unlocked. There's no per-app access control (the oo7 README mentions none either). This is KeyHandoff's present exposure.
- Same-user malware can always: replace the KeyHandoff binary if it's installed under $HOME (AppImage, ~/.local), read files, read the Secret Service, keylog on X11, and wait for the user to unlock and then attach (scope 0) or wrap the launch (any scope). Hardware protection therefore buys: (1) no silent, offline or remote extraction without the user present; (2) a stolen disk or backup is useless; (3) rate-limited guessing; (4) the key isn't trivially readable from storage. It does not buy integrity against a live same-user attacker during an unlocked session.

### Gaps
- Per-distro defaults (Ubuntu, Arch, Pop!_OS 24.04) for ptrace_scope come from secondhand sources plus one local reading. Fedora 45 status is unconfirmed.
- Whether major distro kernels enable secretmem by default in 2026 is unverified.
- No source retrieved this session on Wayland input isolation or X11 keylogging (background knowledge only).

---

## 6. Sandboxing: Flatpak and the Secret portal

### Takeaway
Running *other* apps in Flatpak helps: sandboxed apps get no device nodes, no ptrace of host processes, no access to KeyHandoff's files, and only a filtered session bus. But the common "malware" is a host process (a npm or pip postinstall script, a browser extension's native host, a tampered binary), and those aren't sandboxed. Shipping KeyHandoff as a Flatpak *hurts* TPM access. There's no TPM permission or portal; the only route is `--device=all`, plus libtss2 bundled and /dev/tpmrm0 still gated by the host's `tss` group. The Secret portal gives a Flatpak app a per-app master secret stored in the host keyring. That's convenient but not hardware-backed, and host same-user processes can read the backing keyring.

### Cited Findings
- Flatpak apps get no device nodes except basics like /dev/null. `--device=all` exposes devices broadly, and there are finer categories (`usb`, `input`, etc.) but none for TPM. A maintainer noted that device nodes still obey ordinary Unix permissions. — [Flatpak sandbox permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html); [flatpak issue #3959](https://github.com/flatpak/flatpak/issues/3959); [flatpak issue #3016](https://github.com/flatpak/flatpak/issues/3016)
- `org.freedesktop.portal.Secret.RetrieveSecret(fd, options)` writes to a caller-supplied fd a master secret that is unique per application, stable while installed, and stored by typical backends in the user's keyring keyed by app ID. Its format is opaque and may be short, so the app should KDF it. The page makes no further security claims. — [XDG Desktop Portal: Secret](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Secret.html)
- oo7 implements the portal backend (`oo7-portal`, `org.freedesktop.impl.portal.Secret`). — [linux-credentials/oo7](https://github.com/linux-credentials/oo7)

### Inferences
- [bg] libsecret and oo7, when sandboxed, use a file-backend keyring in ~/.var/app/<id>/data/keyrings encrypted with the portal secret. Other Flatpak apps can't reach it. A host same-user process can read both the file and (via org.freedesktop.secrets) the portal's master secret item, so the protection against host malware is nil.
- [bg] A Flatpak KeyHandoff with `--device=all` would also expose cameras, input and other device nodes, and Flathub reviewers dislike it. A TPM-using Flatpak would also need libtss2 in the manifest. Recommendation: native packages (.deb/.rpm/AUR) for the TPM path. If a Flatpak is offered, it falls back to Argon2id or FIDO2 (FIDO2 hidraw needs `--device=all` too, unless a USB/hidraw permission suffices; unverified).
- [bg] The Flatpak sandbox (bubblewrap with seccomp) blocks sandboxed apps from ptrace outside the sandbox, and their PID namespace hides host processes. So malware *inside* a Flatpak can't attach to host KeyHandoff, even at ptrace_scope 0. Running untrusted apps (chat clients, games) as Flatpaks thus meaningfully reduces exposure, but it's a user choice, not something KeyHandoff controls.
- Snap: not researched. [bg] The snapd `tpm` interface exists (super-privileged), so snaps may have a cleaner TPM route than Flatpak; unverified.

### Gaps
- No primary source on a Flatpak TPM portal (none appears to exist). No source on Flathub policy for `--device=all` in security apps.
- Snap `tpm` interface not verified.

---

## 7. No-TPM fallback, and concrete Rust crates

### Takeaway
Without a TPM, the only cryptographic prompt-gates are (a) an Argon2id passphrase-derived wrapping key (offline brute force limited only by passphrase strength; no hardware rate limit) and (b) a FIDO2 security key using the hmac-secret extension (a real hardware-bound secret released per touch or PIN). GNOME Keyring's own encryption is passphrase-based at rest and gives nothing against same-user access once unlocked. Fingerprint-backed keys don't exist on Linux.

### Cited Findings
- crates.io (queried 2026-10-31):
  - `argon2` 0.6.0 (2026-08-27, pure Rust, RustCrypto)
  - `secret-service` 5.2.0 and `keyring` 4.2.0 (2026-08-29)
  - `oo7` 0.6.0 (2026-09-21, Rust Secret Service client/provider)
  - `nix` 0.31.3 (2026-05-11)
  - `memsec` 0.7.0 (2024-06-06, libsodium-style utils)
  - `zeroize` 1.9.1 (2026-10-06)
  - `secrecy` 0.10.3 (2024-10-09)
  - FIDO2: `ctap-hid-fido2` 3.6.0 (2026-09-05, pure-Rust CTAP), `fido2-rs` 0.6.0 (2026-09-07, bindings to Yubico libfido2), `authenticator` 0.5.0 (2025-10-22, Mozilla's, used by Firefox)
  - No crate named `libfido2` exists.
  — [crates.io API](https://crates.io/api/v1/crates/argon2) (same endpoint per crate name)
- The `tss-esapi` and `tss-esapi-sys` versions are in section 1. — [crates.io](https://crates.io/api/v1/crates/tss-esapi)
- gnome-keyring is encrypted with the login password and unlocked by pam_gnome_keyring at password login, not at fingerprint login. — [Fedora Features/Fingerprint](https://fedoraproject.org/wiki/Features/Fingerprint); [mjg59](https://mjg59.dreamwidth.org/68537.html)
- systemd uses Argon2id for its PIN path, with at least 64 MiB and a 2 s target, which is a reasonable calibration reference for a passphrase KDF. — [systemd-cryptenroll(1)](https://www.freedesktop.org/software/systemd/man/latest/systemd-cryptenroll.html)

### Inferences
- [bg] FIDO2 hmac-secret: at enrollment, create a credential with the `hmac-secret` extension. At unlock, `getAssertion` with a 32-byte salt returns HMAC-SHA-256(credRandom, salt), a key that never exists without the device and touch (UP) and optionally the device PIN or biometric (UV). The device enforces its own PIN retry limit (8 on CTAP2, with power-cycle required after 3). This is the strongest no-TPM, prompt-gated option and is used by systemd-cryptenroll `--fido2-device` and age-plugin-fido2-hmac. Caveats: on Linux, /dev/hidraw access for FIDO keys is granted via udev `uaccess` rules (libfido2 / systemd ship them), so any same-user process can also talk to the key. But each assertion needs a physical touch, so malware can't silently extract it and must trick the user into touching. Users also need to own a key; that's optional hardware, not universal.
- [bg] Crate choice for FIDO2: `fido2-rs` (libfido2 C, mature, the same library systemd uses) vs `ctap-hid-fido2` (pure Rust, supports hmac-secret). Verify hmac-secret support in whichever is picked. Mozilla `authenticator` is WebAuthn-focused.
- Argon2id fallback design: wrap the master key with a key from Argon2id(passphrase, salt, m ≥ 64–256 MiB, t calibrated to around 1 s). Store only the wrapped key, cache the unwrapped key in mlock'd memory (`memsec` or `nix::sys::mman::mlock`, zeroized via `zeroize` or `secrecy`) for the session, and drop it after the idle timeout. Against same-user malware this is better than today (the key isn't readable from storage without the passphrase), but the wrapped blob plus offline guessing is unlimited, so it needs a strong passphrase, not a PIN.
- Optional machine-binding without a TPM: also mix in a secret stored in the Secret Service or a 0600 file. That doesn't stop same-user malware, but it makes a stolen backup of the wrapped blob alone insufficient.
- [bg] Hardening crates: `nix` exposes `prctl` (`nix::sys::prctl::set_dumpable(false)`) and `mlock`/`madvise`. memfd_secret has no nix wrapper as far as known; call `libc::syscall(libc::SYS_memfd_secret, 0)` and fall back on ENOSYS.
- Suggested Linux tiering for KeyHandoff:
  1. TPM 2.0 + Argon2id-hardened PIN (if /dev/tpmrm0 is accessible).
  2. FIDO2 hmac-secret (if a key is present and the user opts in).
  3. Argon2id passphrase.
  4. Today's Secret Service or file (explicitly labelled "no prompt protection").
  In all tiers: unlock once per session, idle-timeout zeroize, PR_SET_DUMPABLE 0 in the core process, key never in the WebView, and an X11 warning.

### Gaps
- Did not verify hmac-secret support or API maturity for `fido2-rs` and `ctap-hid-fido2` specifically.
- No independent security analysis of GNOME Keyring's at-rest crypto was retrieved this session.
- `memsec` was last released in 2024. Its maintenance status is unclear; `zeroize` + `nix` + `libc` may suffice.
