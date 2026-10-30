//! One running KeyHandoff per profile. A second launch (double-clicking a
//! `.handoff` file while the app runs) passes its file paths to the first over
//! a local named pipe, then exits. The pipe's default security lets only this
//! user (and admins) write to it, and remote clients are rejected.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::FromRawHandle;
use std::path::PathBuf;
use std::thread;

use ::windows::core::HSTRING;
use ::windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_CONNECTED};
use ::windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND};
use ::windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
    PIPE_WAIT,
};

const BUFFER: u32 = 4096;

fn pipe(profile: &str) -> String {
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
    let name = HSTRING::from(pipe(profile));
    let spawned = thread::Builder::new().name("launches".into()).spawn(move || {
        let mut first = true;
        loop {
            let flags = if first { PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE } else { PIPE_ACCESS_INBOUND };
            first = false;
            // SAFETY: plain pipe creation; the handle is owned below.
            let handle = unsafe {
                CreateNamedPipeW(
                    &name,
                    flags,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    PIPE_UNLIMITED_INSTANCES,
                    0,
                    BUFFER,
                    0,
                    None,
                )
            };
            if handle.is_invalid() {
                eprintln!("keyhandoff: no single-instance pipe: {}", ::windows::core::Error::from_thread());
                return;
            }
            // SAFETY: a client may connect between create and connect; that is ERROR_PIPE_CONNECTED.
            let connected = match unsafe { ConnectNamedPipe(handle, None) } {
                Ok(()) => true,
                Err(e) => e.code() == ERROR_PIPE_CONNECTED.to_hresult(),
            };
            if !connected {
                // SAFETY: we own the handle.
                let _ = unsafe { CloseHandle(handle) };
                continue;
            }
            // SAFETY: the File takes ownership of the handle and closes it.
            let stream = unsafe { File::from_raw_handle(handle.0) };
            let files = BufReader::new(stream).lines().map_while(Result::ok).take_while(|l| !l.is_empty()).map(PathBuf::from).collect();
            on_launch(files);
        }
    });
    if let Err(e) = spawned {
        eprintln!("keyhandoff: no single-instance listener: {e}");
    }
    Ok(())
}
