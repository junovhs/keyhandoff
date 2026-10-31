//! Linux backends: X11 clipboard, Secret Service master key, GTK drag-out,
//! a Unix socket that keeps one instance per profile, and one that programs
//! ask for kept keys over.

mod clipboard;
mod drag;
mod instance;
mod keys;
mod protect;

pub use clipboard::{arm_capture, offer_file, read_clipboard};
pub use drag::drag_file;
pub use instance::{forward_to_running, listen_for_launches};
pub use keys::listen_for_keys;
pub use protect::master_key;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

/// Create a directory only this user can enter.
pub fn private_dir(dir: &Path) -> std::io::Result<()> {
    fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
}

/// Write a file only this user can read, replacing it atomically.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut f = OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn a_running_clipboard_app_is_named_and_gone_once_closed() {
        // A stand-in process whose name is a known clipboard manager.
        let dir = std::env::temp_dir().join(format!("handoff-apps-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("copyq");
        fs::copy("/bin/sleep", &fake).unwrap();
        let mut child = Command::new(&fake).arg("30").spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(clipboard_apps().contains(&"CopyQ".to_string()));
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!clipboard_apps().contains(&"CopyQ".to_string()));
        fs::remove_dir_all(dir).unwrap();
    }
}

/// Serve a Unix socket at `path`, only this user can connect, on a thread
/// called `thread`, handing each client to `on_client`. Launch forwarding and
/// key requests share this. A stale socket from an earlier run is replaced.
fn serve_socket(path: &Path, thread: &str, mut on_client: impl FnMut(UnixStream) + Send + 'static) -> Result<(), String> {
    let _ = fs::remove_file(path);
    let listener = UnixListener::bind(path).map_err(|e| e.to_string())?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    std::thread::Builder::new()
        .name(thread.into())
        .spawn(move || listener.incoming().flatten().for_each(&mut on_client))
        .map(drop)
        .map_err(|e| e.to_string())
}

/// Per-login memory-backed directory (`$XDG_RUNTIME_DIR`), wiped at logout.
pub fn runtime_dir() -> PathBuf {
    dirs::runtime_dir().unwrap_or_else(std::env::temp_dir)
}

/// Clipboard managers that keep what you copy, by `/proc/<pid>/comm` name.
const CLIPBOARD_APPS: &[(&str, &str)] = &[
    ("gpaste-daemon", "GPaste"),
    ("copyq", "CopyQ"),
    ("klipper", "Klipper"),
    ("clipit", "ClipIt"),
    ("diodon", "Diodon"),
    ("parcellite", "Parcellite"),
    ("xfce4-clipman", "Clipman"),
    ("greenclip", "Greenclip"),
    ("clipmenud", "clipmenu"),
    ("clipcatd", "Clipcat"),
    ("qlipper", "Qlipper"),
];

/// Known clipboard managers running now, from the process list. Clipboard
/// extensions inside a desktop shell don't show up as processes.
pub fn clipboard_apps() -> Vec<String> {
    let running = fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit())))
        .filter_map(|e| fs::read_to_string(e.path().join("comm")).ok())
        .map(|comm| comm.trim_end().to_string());
    super::known_apps(running, CLIPBOARD_APPS)
}

/// The login name; the screens let you change the name you go by.
pub fn user_name() -> Option<String> {
    std::env::var("USER").ok()
}
