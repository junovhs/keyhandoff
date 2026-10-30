//! One running KeyHandoff per profile. A second launch (double-clicking a
//! `.handoff` file while the app runs) passes its file paths to the first over
//! a Unix socket in the runtime directory, then exits.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;

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
    let path = socket(profile);
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
    thread::Builder::new()
        .name("launches".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let files = BufReader::new(stream)
                    .lines()
                    .map_while(Result::ok)
                    .take_while(|l| !l.is_empty())
                    .map(PathBuf::from)
                    .collect();
                on_launch(files);
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
