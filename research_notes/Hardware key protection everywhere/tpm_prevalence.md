# TPM 2.0 prevalence on real desktop and laptop PCs (as of late October 2026), detection, and turning it on

Research date: 2026-10-31. Every figure carries its data date. "Measured" means survey or telemetry data. "Estimate" means my inference from that data.

Local evidence from the developer's own machine (command output on 2026-10-31; a primary source, cited as "[local check]"):
- Board: ASRock X570 Phantom Gaming 4. BIOS P2.20, dated 2019-11-25. CPU: AMD Ryzen 9 3900X. OS: Pop!_OS 22.04 LTS.
- The kernel has the TPM drivers built in: `CONFIG_TCG_TPM=y`, `CONFIG_TCG_TIS=y`, `CONFIG_TCG_CRB=y`. Even so, `/dev/tpm*` does not exist and `/sys/class/tpm` is empty. The kernel log says `ima: No TPM chip found, activating TPM-bypass!`.
- So the firmware is not exposing a TPM to the OS. This CPU and chipset do support AMD fTPM (ASRock lists fTPM on AM4 300-series and newer, see below). The likely cause is that fTPM is switched off in a pre-Windows-11 BIOS. It is not missing hardware and not a driver problem.
- `/lib/udev/rules.d/60-tpm-udev.rules` (Debian/Ubuntu package `tpm-udev`) sets `tpmrm[0-9]*` to `MODE="0660", OWNER="tss", GROUP="tss"`. The `tss` group exists but has no members. User `juno` is not in it (`groups=juno,adm,sudo,plugdev,kvm,lpadmin,jellyfin`). **Even after fTPM is turned on, a normal-user process on this machine could not open /dev/tpmrm0 without a group change.**

## 1. What share of Windows PCs in use have TPM 2.0 enabled?

### Takeaway
About 69–73% of desktop Windows installs ran Windows 11 in 2026 (StatCounter, via press), and about 70–71% of Steam users did. Nearly all of those have TPM 2.0 present and enabled, because Windows 11 requires it for a supported install. Windows 10 still held about 24–30% after its end of support. That share is skewed toward machines that fail Windows 11 checks, a large slice of them because the TPM is absent or switched off.

### Cited Findings
- Steam Hardware Survey, June 2026: Windows 11 at 70.44% of Steam users, Windows 10 at 23.56% (measured) — [Guru3D](https://www.guru3d.com/story/windows-11-tops-on-steam-as-amd-continues-closing-cpu-gap/); first time over 70% — [Windows Central](https://www.windowscentral.com/gaming/pc-gaming/windows-11-new-milestone-steam-over-70-percent-share)
- Steam, August 2026: Windows 11 at 70.97%. This comes from a single secondary source and was not checked against Valve — [Tech Insider](https://tech-insider.org/steam-hardware-survey-august-2026/)
- Steam, January 2026: Windows 11 at 66.71%; April 2026: 67.74% — [GeekChamp](https://geekchamp.com/?p=1095342), [Gameluster](https://gameluster.com/valve-steam-users-windows-11-67-percent/), [Windows Report (Apr 2026)](https://windowsreport.com/valve-steam-survey-april-2026-windows-11-grows-mid-range-pcs-still-dominate/). Gigazine flagged that a January 2026 swing in Windows 10 share may reflect a change in who answered the survey, not real usage — [Gigazine](https://gigazine.net/gsc_news/en/20260303-steam-survey)
- StatCounter (desktop Windows versions, worldwide), February 2026: Windows 11 at 72.57–72.78%, Windows 10 at 26.27–26.5%. Outlets disagree slightly — [PCWorld](https://www.pcworld.com/article/3075245/windows-11-market-share-keeps-climbing-despite-all-the-issues.html), [Windows Report](https://windowsreport.com/windows-11-market-share-explodes-to-72-78-in-february-2026-leaving-windows-10-at-26-27)
- StatCounter, June 2026: Windows 11 at about 69.9%, Windows 10 at about 28.1–28.2% (Windows 10 was about 49% a year earlier) — [MedhaCloud](https://medhacloud.com/blog/windows-market-share). July 2026: Windows 11 at 68.93%, Windows 10 at 29.83%, single secondary source — [TechBloat](https://www.techbloat.com/?p=1902362). Germany lost Windows 11 share from April to May 2026 — [PCWelt](https://www.pcwelt.de/?p=3155156). I could not load StatCounter's own worldwide page.
- Microsoft calls TPM 2.0 "a non-negotiable standard for the future of Windows" (blog post by Steven Hosking, December 2024) — [Windows Latest](https://windowslatest.com/2024/12/04/microsoft-windows-11-tpm-is-a-must-non-negotiable-for-upgrades-from-windows-10/amp), [Windows Central](https://windowscentral.com/software-apps/windows-11/microsoft-makes-tpm-2-0-upgrade-requirement-non-negotiable)
- Windows 10 support ended on October 14, 2025. Microsoft's own TPM page says so — [Microsoft Support: Enable TPM 2.0](https://support.microsoft.com/en-us/windows/enable-tpm-2-0-on-your-pc-1fd5a332-360d-4f46-a1e7-ae6b0c90645c). Paid consumer Extended Security Updates run through October 2026 — [TechRadar](https://www.techradar.com/computing/windows/microsoft-stubbornly-sticks-to-windows-11s-tpm-security-requirement-and-risks-annoying-windows-10-users)
- Up to about 400 million devices were considered not normally upgradable to Windows 11 (iFixit figure via Born City). 43% of those reportedly fail on TPM 2.0, which is roughly 170M — [Born City](https://borncity.com/news/windows-11-tpm-2-0-und-secure-boot-schliessen-bis-zu-400-millionen-geraete-aus/), [Born City](https://borncity.com/news/windows-10-400-millionen-pcs-koennen-nicht-auf-windows-11-upgraden/). Canalys projected up to 240M PCs might be abandoned over Windows 11 compatibility (all causes, not just TPM) — [TechRadar](https://www.techradar.com/news/a-whole-load-of-pcs-still-dont-qualify-for-windows-11). Computerwoche reported about one billion Windows 10 PCs still in use around end of support — [Computerwoche](https://www.computerwoche.de/article/4098546/noch-eine-milliarde-windows-10-pcs-im-einsatz.html)
- Enterprise fleet telemetry (Lansweeper, 2021, about 30M Windows devices across 60,000 organisations), physical workstations (measured): 52.55% had a TPM that met Windows 11 needs, over 19% failed the TPM test, and **28% were "not TPM compatible or did not have it enabled"**. Virtual workstations: only **0.23%** had TPM 2.0 enabled — [HotHardware](https://amp.hothardware.com/news/new-survey-confirms-over-half-of-business-pcs-cant-install-windows-11), [The Register (2021-09-30)](https://www.theregister.com/2021/09/30/windows_11_workstations/), [MyTechDecisions](https://mytechdecisions.com/it-infrastructure/half-of-enterprise-workstations-not-eligible-for-windows-11-upgrade/)
- Lansweeper follow-up, October 2022: 14.66% of systems lacked TPM support, and 20.77% failed the TPM test or did not have TPM enabled — [Born City (2022-10-15)](https://borncity.com/win/2022/10/15/lansweeper-windows-11-readyness-test-auch-im-oktober-2022-fast-50-der-systeme-nicht-kompatibel/)
- TPM 2.0 is "present on virtually all modern PCs". In practice the requirement blocks Windows 11 on CPUs made before about 2018 — [TechRadar](https://www.techradar.com/computing/windows/microsoft-stubbornly-sticks-to-windows-11s-tpm-security-requirement-and-risks-annoying-windows-10-users)

### Inferences
- **Windows 11 installs (about 70% of Windows)**: TPM 2.0 is present and enabled on almost all of them. The exception is a small share installed with requirement bypasses (Rufus or registry). I found no figure for that share.
- **Windows 10 installs (about 24–30% of Windows in mid-2026)**: Lansweeper's 2021–22 split (about 15% no TPM support vs. about 6% present but failing or not enabled, from the 14.66% / 20.77% figures) suggests the "absent" group is larger than "off" in managed fleets. The machines still on Windows 10 in 2026 are mostly the ones that could not upgrade, so absent or 1.2-only TPMs are over-represented there. DIY gaming desktops with fTPM switched off are the main "present but off" group.
- **Estimate, all Windows PCs in use, late 2026**: about 72–78% TPM 2.0 present and enabled; about 5–10% present but disabled in firmware (mostly AMD AM4 or Intel 8th–10th gen DIY boards and some older OEM business machines); about 12–20% absent or TPM 1.2 only (roughly pre-2016 hardware, VMs without vTPM). Low confidence on the split between "off" and "absent".

### Gaps
- I found no measured 2025–2026 figure for "TPM present but disabled" among home PCs. Lansweeper combines "not compatible" with "not enabled", and its data is from 2021–22 and enterprise-only.
- I found no figure for the share of Windows 11 installs made via TPM bypass.
- StatCounter's primary page could not be loaded; its 2026 figures come from press reports that differ by a few tenths of a point.

## 2. Firmware TPMs (AMD fTPM, Intel PTT): availability, default-off on DIY boards, known problems

### Takeaway
Firmware TPMs are on essentially every Ryzen platform (AM4 300-series chipsets onward) and Intel desktop platforms from about the 100/200-series era onward. Before mid-2021, DIY motherboards commonly shipped with them switched off. ASUS, ASRock, Gigabyte and MSI shipped BIOS updates that turned them on by default. On AMD there are three known problems: (a) a 2022 stutter bug, fixed in AGESA 1.2.0.7; (b) BIOS updates or CPU swaps that wipe fTPM contents ("fTPM NV corrupted… Press Y to reset"); (c) a fTPM reset and BitLocker bug that AMD addressed in 2025, with some board makers slow to ship the fix.

### Cited Findings
- ASRock lists fTPM on AMD AM4 300-series and newer chipsets and TRX40, and PTT on Intel X299, B250 and newer — [ASRock news (Windows 11 list)](https://asrock.com/news/index.asp?iD=4696), [Vortez](https://www.vortez.net/news_story/asrock_reveals_windows_11_compatibility_list.html)
- Custom-built PCs often shipped with firmware TPM present but off. A BIOS option enables it, and many PCs failing the Windows 11 check pass once it is on (2021) — [Tom's Hardware](https://www.tomshardware.com/uk/news/windows-11-tpm-enable-bios-uefi)
- ASUS released BIOS updates that turn TPM on by default ("BIOS version default support Windows 11") across many models (2021) — [XDA](https://www.xda-developers.com/asus-is-updating-its-motherboard-models-to-enable-tpm-for-windows-11/), [PC Gamer](https://pcgamer.com/asus-tpm-motherboard-bios-update). ASUS also documents how to turn TPM back off after those BIOS versions; a manual "Discrete TPM" choice can revert to firmware TPM on reboot — [ASUS FAQ 1047459](https://www.asus.com/support/FAQ/1047459)
- Gigabyte AMD menu path: Settings → Miscellaneous → AMD CPU fTPM → Enabled. Some boards show no fTPM option until the BIOS is updated — [Industrial Monitor Direct guide](https://industrialmonitordirect.com/blogs/knowledgebase/enable-tpm-20-for-windows-11-step-by-step-guide) (secondary)
- Ryzen 1000 / Threadripper 1000 are not on Microsoft's Windows 11 CPU list (as of 2021-07-01), even though they have fTPM — [ASRock news](https://www.asrock.com/news/index.de.asp?iD=4699)
- **AMD fTPM stutter**: Windows' frequent polling of the fTPM caused intermittent stutter and audio glitches. AMD acknowledged it in 2022 and shipped AGESA V2 PI 1.2.0.7 to board vendors to fix it — [Guru3D](https://www.guru3d.com/story/amd-agesa-v2-1-2-7-microcode-to-fix-ftpm-stutters/), [XDA](https://www.xda-developers.com/amd-stuttering-windows-11-10-tpm/), [PCGH](https://www.pcgameshardware.de/Windows-Software-277633/News/BIOS-Update-gegen-Ruckler-auf-PCs-mit-AMD-Ryzen-und-aktiviertem-fTPM-1397096/). Some users (anecdotal, AMD Community) say 1.2.0.7 did not fix it for them; a discrete TPM module is a reported workaround — [AMD Community](https://community.amd.com:443/t5/pc-drivers-software/amd-ftpm-causes-random-stuttering-issue/m-p/624503)
- **fTPM wiped on CPU change or BIOS update**: the boot prompt reads roughly "fTPM NV corrupted or fTPM NV structure changed… Press Y to reset fTPM". Resetting clears the keys stored in the TPM, and BitLocker then needs its recovery key — [Framework Community](https://community.frame.work/t/press-y-to-reset-ftpm/74637), [Level1Techs](https://forum.level1techs.com/t/ftpm-reset-prompt-every-time-the-system-boots/206383), [AMD Community](https://community.amd.com/t5/pc-processors/new-cpu-ftpm-corrupted-error-pressed-n-and-now-no-monitor-signal/m-p/735278)
- **2025 AMD fTPM/BitLocker bug**: AMD advises suspending BitLocker before firmware updates. The update process can erase the TPM outside the Windows APIs. Some board makers did not distribute the fix — [WinFuture](https://winfuture.de/news,152114.html), [heise](https://heise.de/-10482872), [Neowin](https://www.neowin.net/news/amd-finally-clarifies-windows-tpm--bitlocker-bug-that-still-affects-ryzen-cpus/) (403 on fetch; summary from search snippet). ASUS shipped AM4/AM5 BIOS updates for AMD fTPM security vulnerabilities — [Guru3D](https://www.guru3d.com/story/asus-expands-am4-and-am5-bios-updates-to-fix-amd-ftpm-security-vulnerabilities/)
- Microsoft Learn Q&A has users needing the BitLocker recovery key after a BIOS update — [Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/5851643/after-updating-bios-system-required-bitlocker-reco)
- [local check]: the developer's 2019 ASRock X570 BIOS (P2.20) predates both the Windows 11 default-on updates and the 2022 AGESA 1.2.0.7 stutter fix.

### Inferences
- For KeyHandoff, a firmware TPM is **not a durable store**. A BIOS update, a CPU swap or a "Press Y to reset fTPM" wipes any key sealed to it. The vault must always keep a recovery path, such as a passphrase-wrapped copy or a recovery code. The TPM can only be a convenience layer on top of that.
- Telling users "update your BIOS, then enable fTPM" is sound advice for old AM4 boards, since it avoids the stutter. But the update itself resets any existing fTPM contents, so it is safe only if nothing is sealed yet.
- A 2017–2021 DIY AM4 or Intel desktop running a launch-era BIOS most likely has fTPM/PTT off. This matches the developer's machine.

### Gaps
- Exact first generations. From background knowledge, not verified this session: Intel PTT appeared around 4th-gen Core (Haswell, 2013) on some platforms, and AMD fTPM before Ryzen on some APUs. No primary source found. A forum post suggests Skylake "or the generation prior" — [Eleven Forum](https://www.elevenforum.com/t/intel-platform-trust-technology-ptt-disable-or-enable.13696/latest)
- Per-vendor factory defaults for MSI and ASRock before 2021 were not confirmed from vendor pages. The ASRock FAQ page (id=500) returned empty when fetched.
- No measured share of DIY boards still running pre-2021 BIOS versions.

## 3. Linux desktops: how common is a usable /dev/tpmrm0?

### Takeaway
The hardware question is the same as on Windows, but Linux machines lean toward DIY and older repurposed hardware, where the TPM is often off or absent. Even when the TPM is on, Debian/Ubuntu-family distros (Pop!_OS included) make `/dev/tpmrm0` `0660 tss:tss`, and desktop users are not in `tss` by default. An unprivileged app therefore usually **cannot** open it without a one-time `usermod -aG tss` (and re-login) or a custom udev rule. The kernel drivers are rarely the problem: `tpm_crb` and `tpm_tis` are built into mainstream distro kernels.

### Cited Findings
- [local check] Pop!_OS 22.04: `CONFIG_TCG_TPM=y`, `CONFIG_TCG_TIS=y`, `CONFIG_TCG_CRB=y` (built-in, not modules). The udev rule from package `tpm-udev`: `KERNEL=="tpmrm[0-9]*", MODE="0660", OWNER="tss", GROUP="tss"`, and raw `tpm[0-9]*` is `0660` owner tss. The desktop user is not in `tss`.
- Upstream tpm2-tss udev rule (Fedora/RHEL): `/dev/tpmrm0` is tss:tss, `/dev/tpm0` is tss:root. Raw `/dev/tpm0` is restricted deliberately to stop users denial-of-servicing the TPM; users should go through the in-kernel resource manager `/dev/tpmrm0` or tpm2-abrmd — [oss-security 2020-08-06](https://www.openwall.com/lists/oss-security/2020/08/06/2), [oss-security follow-up](https://openwall.com/lists/oss-security/2020/08/06/4)
- NixOS: the `security.tpm2` module sets the tpmrm device group (default `tss`) via a udev rule — [nixpkgs tpm2.nix mirror](https://git.gay/transgirllucy/nixpkgs/src/branch/master/nixos/modules/security/tpm2.nix)
- Pluton's TPM interface needed `tpm_crb` changes (non-standard ACPI start method) that landed in Linux 6.3 (2023). Older kernels do not see a Pluton TPM — [Phoronix](https://phoronix.com/news/Linux-6.3-CRB-TPM2-Pluton), [kernel commit](https://github.com/xanmod/linux/commit/4d2732882703791ea4b670df433f88fc4b40a5cb)
- If the firmware has the TPM off, no `/dev/tpm*` node appears and IMA logs "No TPM chip found" — [local check]

### Inferences
- "Usable by a normal-user app with no setup" on Linux is far lower than "TPM hardware present". Estimate for Linux desktops in late 2026: about 50–65% have a TPM 2.0 enabled in firmware (laptops and recent prebuilts high, DIY desktops lower); about 15–25% have one present but off; about 15–25% have none (old hardware, VMs). Of the enabled group, only a minority let a normal user open `/dev/tpmrm0` out of the box: Debian, Ubuntu, Pop!_OS and Fedora all need `tss` membership. So **an unprivileged app works with no setup on perhaps 5–20% of Linux desktops**. Low confidence; this rests on the permission defaults above, not measurement.
- The app should test whether it can actually open `/dev/tpmrm0`, not just whether the node exists. It should offer a clear one-time fix ("sudo usermod -aG tss $USER, then log out and back in"), or use a polkit/systemd service instead of a raw device.

### Gaps
- No measured data on TPM presence among Linux desktops (no Linux equivalent of the Steam OS-specific TPM stats).
- Arch Wiki TPM page blocked the fetch (Anubis). Not verified this session: Arch's default device group, and whether newer systemd releases (e.g. systemd 256+ with `systemd-tpm2-setup`, `tpm2` udev rules shipped by systemd itself) or Fedora 40+ widened `/dev/tpmrm0` access to logged-in users via uaccess. Worth checking directly in each distro's systemd and tpm2-tss packages.
- Ubuntu 24.04 / Fedora 41+ defaults were not directly inspected.

## 4. Virtual machines, cloud desktops, Macs, Chromebooks, ARM Windows, Pluton

### Takeaway
VMs mostly lack a TPM unless a vTPM is added explicitly (0.23% of enterprise virtual workstations had one in 2021). Microsoft Pluton is a CPU-integrated security processor that can serve as the TPM on Ryzen 6000+, Intel Core Ultra 200V/Series 3, and Snapdragon 8cx Gen 3 / X Series. On Linux, Pluton needs kernel 6.3+. Macs have no TPM.

### Cited Findings
- Lansweeper 2021: only 0.23% of virtual workstations had TPM 2.0 enabled; most need a vTPM added — [HotHardware](https://amp.hothardware.com/news/new-survey-confirms-over-half-of-business-pcs-cant-install-windows-11), [The Register](https://www.theregister.com/2021/09/30/windows_11_workstations/)
- Pluton is on Windows 11 devices with AMD Ryzen 6000, 7000, 8000, 9000 and Ryzen AI; Intel Core Ultra 200V, Ultra Series 3 and Series 3; Qualcomm Snapdragon 8cx Gen 3 and Snapdragon X Series (Microsoft Learn, updated 2026-10-08) — [Microsoft Learn: Pluton](https://learn.microsoft.com/en-us/windows/security/hardware-security/pluton/microsoft-pluton-security-processor)
- Pluton "is designed to provide the functionality of the Trusted Platform Module" and can be "configured as the TPM for the system". Its firmware is loaded from SPI flash and may be updated through Windows Update — [Microsoft Learn: Pluton](https://learn.microsoft.com/en-us/windows/security/hardware-security/pluton/microsoft-pluton-security-processor)
- Pluton exposes a CRB-style TPM2 ACPI device on Ryzen, supported from Linux 6.3; not on EPYC — [Phoronix](https://phoronix.com/news/Linux-6.3-CRB-TPM2-Pluton)

### Inferences
- On Pluton-capable laptops the OEM picks Pluton-as-TPM, AMD fTPM, Intel PTT or a discrete TPM. From the app's view each is just "TPM 2.0", so detection code needs no special case.
- Cloud desktops (Windows 365, Azure Virtual Desktop) and Windows 11 VMs must expose a vTPM to run Windows 11 at all. Linux VMs and older hypervisor setups usually do not.

### Gaps
- Not verified this session (background knowledge only): Hyper-V Gen2, VMware Workstation, VirtualBox 7+, and QEMU/libvirt with swtpm all offer an optional vTPM, off by default for Linux guests. Azure Trusted Launch and Windows 365 provide vTPMs. Intel Macs with T2 and Apple Silicon Macs use the Secure Enclave rather than a TPM, and older Intel Macs have none. Chromebooks use a Google security chip (Cr50/Ti50) that implements TPM functions for ChromeOS but is not available to a third-party desktop app. Each needs a source before it goes into the report.
- Whether Pluton is the default TPM on most shipping Ryzen/Snapdragon laptops (vs. fTPM) was not established.

## 5. How an app detects a TPM

### Takeaway
On Windows, call `Tbsi_GetDeviceInfo` (returns the TPM version, or `TBS_E_TPM_NOT_FOUND`). Then confirm usability by opening the "Microsoft Platform Crypto Provider" with NCrypt and creating a test key. On Linux, read `/sys/class/tpm/tpm0/tpm_version_major` for presence and version, then actually `open()` `/dev/tpmrm0` read-write to test permission.

### Cited Findings
- `TBS_RESULT Tbsi_GetDeviceInfo(UINT32 Size, PVOID Info)` fills a `TPM_DEVICE_INFO` (four UINT32s, including the TPM version). It returns `TBS_SUCCESS` (0), `TBS_E_BAD_PARAMETER` (0x80284002), or `TBS_E_TPM_NOT_FOUND` (0x8028400F, "A compatible Trusted Platform Module (TPM) Security Device cannot be found on this computer"). Tbs.dll, Windows 8+ — [Microsoft Learn: Tbsi_GetDeviceInfo](https://learn.microsoft.com/en-us/windows/win32/api/tbs/nf-tbs-tbsi_getdeviceinfo)
- User-facing checks: Windows Security → Device Security → "Security processor" (missing means the TPM may be disabled; details show "Specification version" 2.0); `tpm.msc` shows "Compatible TPM cannot be found" when it is off — [Microsoft Support: Enable TPM 2.0](https://support.microsoft.com/en-us/windows/enable-tpm-2-0-on-your-pc-1fd5a332-360d-4f46-a1e7-ae6b0c90645c)
- Linux: with no TPM exposed, `/sys/class/tpm/` is empty and there is no `/dev/tpm0` or `/dev/tpmrm0` — [local check]

### Inferences
- Suggested detection states for the UI, in order:
  1. No TPM node or `TBS_E_TPM_NOT_FOUND`: "absent or off". If the CPU is one known to have fTPM/PTT (Ryzen, Intel 6th gen+), say "probably off in firmware".
  2. Version 1.2: "too old".
  3. Present at 2.0 but open or key-create fails: Linux EACCES means "needs permission"; on Windows it is usually a TPM-not-ready or provisioning state.
  4. Key created and used: "usable".
- On Linux the CPU vendor and model in `/proc/cpuinfo` plus DMI board info in `/sys/class/dmi/id/` let the app tell "your Ryzen 9 3900X has fTPM; it's off in your ASRock BIOS" apart from "no TPM".

### Gaps
- `TPM_DEVICE_INFO` field names (`structVersion`, `tpmVersion` = `TPM_VERSION_12`/`TPM_VERSION_20`, `tpmInterfaceType`, `tpmImpRevision`), the `Get-Tpm` output fields (`TpmPresent`, `TpmReady`, `TpmEnabled`…), and the exact NCrypt provider string `MS_PLATFORM_CRYPTO_PROVIDER` ("Microsoft Platform Crypto Provider") are from background knowledge. The fetched page did not include the struct, so they need checking against Microsoft docs.
- `/sys/class/tpm/tpm0/tpm_version_major` (kernel 5.x+) is from background knowledge. The Arch Wiki was blocked, and kernel ABI docs were not fetched.
- A TPM alone does not give a user-presence prompt. On Windows that comes from Windows Hello (NCrypt/KeyCredentialManager); on Linux there is no standard equivalent. Not researched here.

## 6. How to guide a user to turn it on

### Takeaway
Send the user into UEFI setup: on Windows, Settings → Recovery → Advanced startup → UEFI Firmware Settings. Look under "Advanced", "Security" or "Trusted Computing" for one of these vendor names: "AMD fTPM switch", "AMD PSP fTPM", "AMD CPU fTPM", "Intel PTT", "Intel Platform Trust Technology", "Security Device Support", "TPM State". A fresh enable loses nothing. The risks come later: BIOS updates and CPU swaps on AMD can wipe it, and turning it off or clearing it locks BitLocker or sealed keys.

### Cited Findings
- Microsoft's official guide lists the setting names "Security Device", "Security Device Support", "TPM State", "AMD fTPM switch", "AMD PSP fTPM", "Intel PTT", "Intel Platform Trust Technology". It points to "Advanced", "Security" or "Trusted Computing" submenus and gives the path Settings → Recovery → Restart now → Troubleshoot → Advanced options → UEFI Firmware Settings — [Microsoft Support: Enable TPM 2.0](https://support.microsoft.com/en-us/windows/enable-tpm-2-0-on-your-pc-1fd5a332-360d-4f46-a1e7-ae6b0c90645c)
- Gigabyte AMD: Settings → Miscellaneous → AMD CPU fTPM. Some boards need a BIOS update before the option appears. Windows 11 also wants CSM off and Secure Boot on (not needed for TPM itself) — [Industrial Monitor Direct](https://industrialmonitordirect.com/blogs/knowledgebase/enable-tpm-20-for-windows-11-step-by-step-guide)
- Naming differs: PTT on Intel, PSP and/or fTPM on AMD — [Tom's Hardware](https://www.tomshardware.com/uk/news/windows-11-tpm-enable-bios-uefi)
- ASUS "Windows 11 default" BIOS pages and the TPM-toggle FAQ — [ASUS FAQ 1047459](https://www.asus.com/support/FAQ/1047459); ASRock Windows 11 compatibility and BIOS list — [ASRock news](https://asrock.com/news/index.asp?iD=4696)
- Risk: on AMD, BIOS updates and CPU swaps can reset the fTPM, and BitLocker then asks for its recovery key. AMD advises suspending BitLocker before updating — [WinFuture](https://winfuture.de/news,152114.html), [heise](https://heise.de/-10482872), [Framework Community](https://community.frame.work/t/press-y-to-reset-ftpm/74637)

### Inferences
- Turning on a TPM that has never been on is low-risk: no existing keys can be lost. On a Linux-only box BitLocker is not a factor. On Windows, a machine that already has BitLocker and a TPM is not in the "off" group anyway.
- Suggested in-app copy for the developer's case: "Your AMD Ryzen has a built-in security chip (fTPM) that's switched off. Restart into your ASRock BIOS (press F2 or Del at boot) → Advanced → AMD fTPM switch → AMD CPU fTPM → Save and exit." Then on Linux: "add yourself to the tss group." The ASRock path is from background knowledge and should be confirmed against an ASRock manual.
- The best public guides are Microsoft's page above (vendor-neutral) and each board vendor's Windows 11 BIOS page. Linking to Microsoft's page is safer than writing per-vendor steps.

### Gaps
- MSI (typically "Settings → Security → Trusted Computing → AMD fTPM switch / Security Device Support") and exact ASRock menu paths are from background knowledge, not confirmed from vendor pages this session.
- OEM laptop paths (Dell, HP, Lenovo) were not researched. Those machines rarely ship with the TPM off.

## 7. Overall estimate: present and usable, present but disabled, absent

### Takeaway
Estimates, late 2026. Windows: about 75% usable, 5–10% off, 12–20% absent. Linux desktops: about 50–65% have it on in firmware but only about 5–20% are usable by a normal app without setup. All desktops: roughly 70% have a usable TPM 2.0. A hardware-key design must treat "no usable TPM" as a normal case, not an edge case.

### Cited Findings
- Windows 11 is about 69–73% of desktop Windows (StatCounter, Feb–Jul 2026) and about 70–71% of Steam users (Jun–Aug 2026). TPM 2.0 is required for supported installs — see section 1 sources ([PCWorld](https://www.pcworld.com/article/3075245/windows-11-market-share-keeps-climbing-despite-all-the-issues.html), [Guru3D](https://www.guru3d.com/story/windows-11-tops-on-steam-as-amd-continues-closing-cpu-gap/), [Windows Latest](https://windowslatest.com/2024/12/04/microsoft-windows-11-tpm-is-a-must-non-negotiable-for-upgrades-from-windows-10/amp))
- Enterprise 2022 baseline: 14.66% lacked TPM support, 20.77% failed the TPM test or had it off — [Born City](https://borncity.com/win/2022/10/15/lansweeper-windows-11-readyness-test-auch-im-oktober-2022-fast-50-der-systeme-nicht-kompatibel/)
- Linux permission default: `/dev/tpmrm0` is `0660 tss:tss` — [local check], [oss-security](https://www.openwall.com/lists/oss-security/2020/08/06/2)

### Inferences (all estimates, not measured)
| Population (late 2026) | TPM 2.0 present and usable by the app | Present but off, or not accessible | Absent / 1.2 only |
|---|---|---|---|
| (a) Windows PCs in use | ~72–78% | ~5–10% (firmware off) | ~12–20% |
| (b) Linux desktops: hardware on in firmware | ~50–65% | ~15–25% (firmware off) | ~15–25% |
| (b′) Linux desktops: openable by a normal user with no setup | ~5–20% | ~55–70% (off, or needs `tss` group) | ~15–25% |
| (c) All desktops/laptops (Windows ~90%+ weight, macOS and Linux the rest; macOS has no TPM) | ~65–72% | ~8–15% | ~15–25% (includes Macs) |

- Reasoning: the Windows 11 share sets the floor for "usable" on Windows. Lansweeper's split says "absent" beats "off" in fleets. DIY desktops (the developer's own included) are the main "off" group. Linux skews toward DIY and older hardware and adds a permission barrier.
- Design implication: about a quarter to a third of all users, and most Linux users, will need either guidance or a fallback (passphrase or OS keyring). The guided-enable flow mainly pays off for Windows 10 holdouts and DIY desktop and Linux users with Ryzen or Intel 6th gen+ CPUs.

### Gaps
- No primary, measured dataset gives TPM status (on, off, absent) for consumer PCs or Linux in 2025–2026. All splits above are inferred.
- OS desktop share weights (Windows vs. macOS vs. Linux) for the "all desktops" row were not sourced this session.
