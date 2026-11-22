//! macOS backends: the general pasteboard, a Keychain copy of the master key,
//! AppKit drag-out, a Unix socket that keeps one instance per profile, and one
//! that programs ask for kept keys over. Process details come from libproc
//! and `sysctl`, since macOS has no `/proc`.

mod clipboard;
mod drag;
mod instance;
mod keys;
mod process;
mod protect;
pub mod typing;

pub use clipboard::{arm_capture, offer_file, offer_text, read_clipboard};
pub use drag::drag_file;
pub use instance::{forward_to_running, launch_files, listen_for_launches};
pub use keys::{agent, agent_session, connect_keys, descends_from, listen_for_keys, socket as key_socket};
pub use process::{exit_code, hook_env, hooked, ignore_interrupts, is_program, open_url, program_suffixes, same_file};
pub use protect::{forget_master_key, master_key, remember_master_key, remembered_master_key};

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

/// Whether the login session's screen is locked, as the window server
/// reports it (`CGSSessionScreenIsLocked`). False when it can't say.
pub fn screen_locked() -> bool {
    use objc2_core_foundation::{CFBoolean, CFString};
    let Some(session) = objc2_core_graphics::CGSessionCopyCurrentDictionary() else { return false };
    let key = CFString::from_static_str("CGSSessionScreenIsLocked");
    // SAFETY: the session dictionary maps CFString keys to CF objects; the
    // value, if present, is borrowed for the dictionary's lifetime.
    let value = unsafe { session.value((&raw const *key).cast()) };
    if value.is_null() {
        return false;
    }
    // SAFETY: non-null, and this key holds a CFBoolean.
    let flag = unsafe { &*value.cast::<CFBoolean>() };
    flag.value()
}

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

/// The per-user temporary folder (`$TMPDIR`, under `/var/folders`), which
/// only this user can enter. macOS has no `$XDG_RUNTIME_DIR`. Socket paths
/// must fit in 104 bytes, which `$TMPDIR` leaves room for.
pub fn runtime_dir() -> PathBuf {
    std::env::temp_dir()
}

/// Clipboard managers that keep what you copy, by process name.
const CLIPBOARD_APPS: &[(&str, &str)] = &[
    ("Paste", "Paste"),
    ("Maccy", "Maccy"),
    ("Alfred", "Alfred"),
    ("Raycast", "Raycast"),
    ("CopyClip", "CopyClip"),
    ("CopyClip 2", "CopyClip"),
    ("Pastebot", "Pastebot"),
    ("Flycut", "Flycut"),
    ("ClipMenu", "ClipMenu"),
    ("Clipy", "Clipy"),
    ("Jumpcut", "Jumpcut"),
    ("PasteNow", "PasteNow"),
    ("Unclutter", "Unclutter"),
    ("Copy 'Em", "Copy 'Em"),
    ("Clipboard Manager", "Clipboard Manager"),
    ("LaunchBar", "LaunchBar"),
    ("Keyboard Maestro Engine", "Keyboard Maestro"),
    ("Pasty", "Pasty"),
    ("PastePal", "PastePal"),
    ("Yippy", "Yippy"),
];

/// Known clipboard managers running now, from the process list. Spotlight's
/// own clipboard history (macOS 26 and later) isn't a process of its own, so
/// an empty list isn't a promise.
pub fn clipboard_apps() -> Vec<String> {
    super::known_apps(keys::all_pids().into_iter().filter_map(keys::name), CLIPBOARD_APPS)
}

/// The login name; the screens let you change the name you go by.
pub fn user_name() -> Option<String> {
    std::env::var("USER").ok()
}

/// Keep a panel from ever taking keyboard focus, even when shown, so typing
/// never moves focus away from the field it types into. The window is made
/// non-activating: clicking it doesn't bring KeyHandoff forward.
pub fn never_take_focus(panel: &tauri::WebviewWindow) -> tauri::Result<()> {
    panel.set_focusable(false)
}

/// For tests: the backend a profile from before key slots kept its master
/// key in without the Keychain, a `0600` file (so tests never touch the
/// login Keychain; `keychain_remembers_and_forgets` does, on request).
#[cfg(test)]
pub const OLD_KEY_BACKEND: Option<&str> = Some(protect::FILE);

/// Safe Copy can tell when something reads the clipboard while it listens.
/// The general pasteboard keeps its own copy, so reads can't be seen.
pub const CLIPBOARD_BAIT: bool = false;

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn a_running_clipboard_app_is_named_and_gone_once_closed() {
        // A stand-in process whose name is a known clipboard manager.
        let dir = std::env::temp_dir().join(format!("handoff-apps-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("Maccy");
        fs::copy("/bin/sleep", &fake).unwrap();
        let mut child = Command::new(&fake).arg("30").spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(clipboard_apps().contains(&"Maccy".to_string()));
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!clipboard_apps().contains(&"Maccy".to_string()));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn private_files_are_owner_only() {
        let dir = std::env::temp_dir().join(format!("handoff-private-{}", std::process::id()));
        private_dir(&dir).unwrap();
        let file = dir.join("x");
        write_private(&file, b"abc").unwrap();
        assert_eq!(fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        fs::remove_dir_all(dir).unwrap();
    }
}
