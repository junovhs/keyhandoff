//! Windows backends: Safe Copy capture with Win+V history removal, the
//! `CF_HDROP` clipboard, shell drag-out, the DPAPI-wrapped master key, a
//! named pipe that keeps one instance per profile, and one that programs ask
//! for kept keys over.

mod capture;
mod clipboard;
mod drag;
mod instance;
mod keys;
mod protect;

pub use capture::arm_capture;
pub use clipboard::{offer_file, read_clipboard};
pub use drag::drag_file;
pub use instance::{forward_to_running, listen_for_launches};
pub use keys::listen_for_keys;
pub use protect::master_key;

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
