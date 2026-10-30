//! Linux backends: X11 clipboard, Secret Service master key, GTK drag-out,
//! and a Unix socket that keeps one instance per profile.

mod clipboard;
mod drag;
mod instance;
mod protect;

pub use clipboard::{arm_capture, offer_file, read_clipboard};
pub use drag::drag_file;
pub use instance::{forward_to_running, listen_for_launches};
pub use protect::master_key;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
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

/// Per-login memory-backed directory (`$XDG_RUNTIME_DIR`), wiped at logout.
pub fn runtime_dir() -> PathBuf {
    dirs::runtime_dir().unwrap_or_else(std::env::temp_dir)
}

/// The login name; the screens let you change the name you go by.
pub fn user_name() -> Option<String> {
    std::env::var("USER").ok()
}
