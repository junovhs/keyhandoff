//! One running KeyHandoff per profile. Finder doesn't start a second copy
//! when you double-click a `.handoff` file: it tells the running app with an
//! Apple Event, which Tauri hands over as `RunEvent::Opened`. A second launch
//! from a terminal (`keyhandoff key.handoff`) passes its file paths to the
//! first over a Unix socket in the per-user temporary folder, then exits.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

fn socket(profile: &str) -> PathBuf {
    super::runtime_dir().join(format!("handoff-{profile}.sock"))
}

/// Send `files` (possibly none: "show yourself") to a running instance.
/// Returns false when no instance is running.
pub fn forward_to_running(profile: &str, files: &[PathBuf]) -> bool {
    let Ok(mut stream) = UnixStream::connect(socket(profile)) else { return false };
    let mut message = String::new();
    for f in files {
        message.push_str(&f.to_string_lossy());
        message.push('\n');
    }
    message.push('\n');
    stream.write_all(message.as_bytes()).is_ok()
}

/// Listen for later launches. `on_launch` gets each launch's file paths.
pub fn listen_for_launches(profile: &str, on_launch: impl Fn(Vec<PathBuf>) + Send + 'static) -> Result<(), String> {
    super::serve_socket(&socket(profile), "launches", move |stream| {
        let files = BufReader::new(stream).lines().map_while(Result::ok).take_while(|l| !l.is_empty()).map(PathBuf::from).collect();
        on_launch(files);
    })
}

/// Files the system asked the running app to open: Finder's double-click,
/// Open With, or a file dropped on the Dock icon. `Some(vec![])` when the
/// Dock icon was clicked with no window showing ("show yourself").
pub fn launch_files(event: &tauri::RunEvent) -> Option<Vec<PathBuf>> {
    match event {
        tauri::RunEvent::Opened { urls } => Some(urls.iter().filter_map(|u| u.to_file_path().ok()).collect()),
        tauri::RunEvent::Reopen { has_visible_windows: false, .. } => Some(Vec::new()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_launch_hands_its_files_to_the_running_one() {
        let profile = format!("test-launch-{}", std::process::id());
        let (tx, rx) = std::sync::mpsc::channel();
        listen_for_launches(&profile, move |files| tx.send(files).unwrap()).unwrap();
        let files = [PathBuf::from("/tmp/Key for Ann.handoff"), PathBuf::from("/tmp/b.handoff")];
        assert!(forward_to_running(&profile, &files));
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(), files);
        let _ = std::fs::remove_file(socket(&profile));
        assert!(!forward_to_running(&profile, &files), "no instance, no forward");
    }
}
