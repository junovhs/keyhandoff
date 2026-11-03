//! Windows backends: Safe Copy capture with Win+V history removal, the
//! `CF_HDROP` clipboard, shell drag-out, the DPAPI-wrapped master key, a
//! named pipe that keeps one instance per profile, and one that programs ask
//! for kept keys over.

mod capture;
mod clipboard;
mod drag;
mod instance;
mod key_stream;
mod keys;
mod pipe;
mod protect;

pub use capture::arm_capture;
pub use clipboard::{offer_file, offer_text, read_clipboard};
pub use drag::drag_file;
pub use instance::{forward_to_running, listen_for_launches};
pub use keys::{agent, connect_keys, descends_from, listen_for_keys};
pub use protect::{forget_master_key, master_key, remember_master_key, remembered_master_key};

/// Whether this session's screen is locked (`WTS_SESSIONSTATE_LOCK`, which
/// reads correctly from Windows 8 on). False when Windows won't say.
pub fn screen_locked() -> bool {
    use ::windows::core::PWSTR;
    use ::windows::Win32::System::RemoteDesktop::{
        WTSFreeMemory, WTSQuerySessionInformationW, WTSSessionInfoEx, WTSINFOEXW, WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION,
        WTS_SESSIONSTATE_LOCK,
    };
    let mut buf = PWSTR::null();
    let mut len = 0u32;
    // SAFETY: Windows allocates `buf` on success; we read one WTSINFOEXW from
    // it (when it's that big) and free it with WTSFreeMemory.
    unsafe {
        if WTSQuerySessionInformationW(Some(WTS_CURRENT_SERVER_HANDLE), WTS_CURRENT_SESSION, WTSSessionInfoEx, &mut buf, &mut len).is_err() {
            return false;
        }
        let locked = len as usize >= std::mem::size_of::<WTSINFOEXW>() && {
            let info = &*(buf.0 as *const WTSINFOEXW);
            info.Level == 1 && info.Data.WTSInfoExLevel1.SessionFlags == WTS_SESSIONSTATE_LOCK as i32
        };
        WTSFreeMemory(buf.0 as *mut _);
        locked
    }
}

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use ::windows::core::PWSTR;
use ::windows::Win32::Security::Authentication::Identity::{GetUserNameExW, NameDisplay};

/// `%APPDATA%` is already private to the Windows user.
pub fn private_dir(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)
}

/// Write a file, replacing it atomically: temp file, flush, rename.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// `%TEMP%`, per user. Files you send are written here.
pub fn runtime_dir() -> PathBuf {
    std::env::temp_dir()
}

/// Clipboard managers that keep what you copy, by lowercase executable name.
const CLIPBOARD_APPS: &[(&str, &str)] = &[
    ("ditto.exe", "Ditto"),
    ("copyq.exe", "CopyQ"),
    ("clipclip.exe", "ClipClip"),
    ("clipboardfusion.exe", "ClipboardFusion"),
    ("clipangel.exe", "ClipAngel"),
    ("arsclip.exe", "ArsClip"),
    ("clipmate.exe", "ClipMate"),
    ("clipdiary.exe", "Clipdiary"),
    ("1clipboard.exe", "1Clipboard"),
    ("clcl.exe", "CLCL"),
    ("clipx.exe", "ClipX"),
    ("clibor.exe", "Clibor"),
    ("phraseexpress.exe", "PhraseExpress"),
];

/// Known clipboard managers running now, from the process list. Win+V
/// history is Windows itself and is handled after each catch instead.
pub fn clipboard_apps() -> Vec<String> {
    use ::windows::Win32::Foundation::CloseHandle;
    use ::windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    let mut running = Vec::new();
    // SAFETY: a process snapshot walk; `entry.dwSize` is set and the handle is closed.
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return Vec::new() };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let end = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            running.push(String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase());
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = CloseHandle(snapshot);
    }
    super::known_apps(running.into_iter(), CLIPBOARD_APPS)
}

/// The Windows display name ("Ann Hall"), else the account name.
pub fn user_name() -> Option<String> {
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` holds `len` units; on success `len` is the length written.
    let ok = unsafe { GetUserNameExW(NameDisplay, Some(PWSTR(buf.as_mut_ptr())), &mut len) };
    if ok && len > 0 {
        return Some(String::from_utf16_lossy(&buf[..len as usize]));
    }
    std::env::var("USERNAME").ok()
}
