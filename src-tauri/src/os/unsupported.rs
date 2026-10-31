//! Placeholder for operating systems whose backends are not built yet
//! (macOS). Everything refuses.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use super::{Captured, Clip, MasterKey, ServeKeys};

const NOT_YET: &str = "KeyHandoff does not support this operating system yet.";

pub fn arm_capture(_timeout: Duration, _cancel: Arc<AtomicBool>, _done: impl FnOnce(Captured) + Send + 'static) -> Result<(), String> {
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
