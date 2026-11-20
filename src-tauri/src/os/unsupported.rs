//! Placeholder for operating systems whose backends are not built yet
//! (macOS). Everything refuses.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use super::{Captured, Clip, MasterKey, ServeKeys};

const NOT_YET: &str = "KeyHandoff does not support this operating system yet.";

pub fn arm_capture(
    _timeout: Duration,
    _cancel: Arc<AtomicBool>,
    _done: impl FnOnce(Captured) + Send + 'static,
    _read: impl Fn() + Send + 'static,
) -> Result<(), String> {
    Err(NOT_YET.into())
}

pub fn offer_file(_file: &Path, _text: String) -> Result<(), String> {
    Err(NOT_YET.into())
}

pub fn offer_text(_text: String) -> Result<(), String> {
    Err(NOT_YET.into())
}

pub fn read_clipboard() -> Result<Clip, String> {
    Err(NOT_YET.into())
}

pub fn drag_file(_window: &tauri::WebviewWindow, _file: &Path, _text: String) -> Result<(), String> {
    Err(NOT_YET.into())
}

pub fn master_key(_dir: &Path, _profile: &str, _backend: Option<&str>) -> Result<(MasterKey, &'static str), String> {
    Err(NOT_YET.into())
}

pub fn screen_locked() -> bool {
    false
}

pub fn remember_master_key(_dir: &Path, _profile: &str, _key: &MasterKey, _prefer: Option<&str>) -> Result<&'static str, String> {
    Err(NOT_YET.into())
}

pub fn remembered_master_key(_dir: &Path, _profile: &str, _backend: &str) -> Result<Option<MasterKey>, String> {
    Err(NOT_YET.into())
}

pub fn forget_master_key(_dir: &Path, _profile: &str, _backend: &str) -> Result<(), String> {
    Err(NOT_YET.into())
}

pub fn forward_to_running(_profile: &str, _files: &[PathBuf]) -> bool {
    false
}

pub fn listen_for_launches(_profile: &str, _on_launch: impl Fn(Vec<PathBuf>) + Send + 'static) -> Result<(), String> {
    Ok(())
}

pub fn listen_for_keys(_profile: &str, _serve: ServeKeys) -> Result<(), String> {
    Ok(())
}

pub fn clipboard_apps() -> Vec<String> {
    Vec::new()
}

pub fn private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

pub fn runtime_dir() -> PathBuf {
    std::env::temp_dir()
}

pub fn user_name() -> Option<String> {
    std::env::var("USER").ok()
}

pub fn connect_keys(_profile: &str) -> std::io::Result<Box<dyn crate::os::KeyStream>> {
    Err(std::io::Error::other("not supported on this system"))
}

pub fn descends_from(_pid: u32, _ancestor: u32) -> bool {
    false
}

pub fn agent(_pid: u32) -> Option<String> {
    None
}

pub fn agent_session(_pid: u32, _expected: &str) -> Option<crate::os::AgentProcess> {
    None
}

impl crate::os::Peer {
    pub fn unattended_origin(&self) -> Option<&'static str> {
        None
    }
}

pub(crate) use super::typing_unsupported as typing;

// keyhandoff run: no hook is built for this system yet.

pub fn hooked(_program: &std::path::Path) -> bool {
    false
}

pub fn hook_env(_profile: &str, _token: &str) -> Result<Vec<(String, zeroize::Zeroizing<String>)>, String> {
    Err("Placeholders need KeyHandoff's TLS hook, which isn't built for this system yet.".into())
}

pub fn is_program(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

pub fn ignore_interrupts() {}

pub fn exit_code(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status.code().or(status.signal().map(|s| 128 + s)).unwrap_or(1)
}

pub fn program_suffixes() -> Vec<String> {
    vec![String::new()]
}

pub fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    matches!((a.metadata(), b.metadata()), (Ok(a), Ok(b)) if a.dev() == b.dev() && a.ino() == b.ino())
}

/// macOS's `open`; the only other system this builds for so far.
pub fn open_url(url: &str) -> std::io::Result<()> {
    std::process::Command::new("open").arg(url).spawn().map(|_| ())
}

pub fn never_take_focus(_panel: &tauri::WebviewWindow) -> tauri::Result<()> {
    Ok(())
}
