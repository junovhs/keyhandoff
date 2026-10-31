//! One running KeyHandoff per profile. A second launch (double-clicking a
//! `.handoff` file while the app runs) passes its file paths to the first over
//! a local named pipe (`pipe.rs`), then exits.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

const BUFFER: u32 = 4096;

pub(super) fn pipe(profile: &str) -> String {
    let user: String = std::env::var("USERNAME").unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    format!(r"\\.\pipe\handoff-{user}-{profile}")
}

/// Send `files` (possibly none: "show yourself") to a running instance.
/// Returns false when no instance is running.
pub fn forward_to_running(profile: &str, files: &[PathBuf]) -> bool {
    let Ok(mut stream) = OpenOptions::new().write(true).open(pipe(profile)) else { return false };
    let mut message = String::new();
    for f in files {
        message.push_str(&f.to_string_lossy());
        message.push('\n');
    }
    message.push('\n');
    stream.write_all(message.as_bytes()).is_ok()
}

/// Listen for later launches. `on_launch` gets each launch's file paths.
/// A pipe that can't be made only costs single-instance, so it is logged,
/// not fatal.
pub fn listen_for_launches(profile: &str, on_launch: impl Fn(Vec<PathBuf>) + Send + 'static) -> Result<(), String> {
    let served = super::pipe::serve(&pipe(profile), false, BUFFER, "launches", move |stream| {
        let files = BufReader::new(stream).lines().map_while(Result::ok).take_while(|l| !l.is_empty()).map(PathBuf::from).collect();
        on_launch(files);
    });
    if let Err(e) = served {
        eprintln!("keyhandoff: no single-instance listener: {e}");
    }
    Ok(())
}
