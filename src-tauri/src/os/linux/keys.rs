//! Programs ask for kept keys over a Unix socket in the runtime directory,
//! which only this user can enter. The kernel reports the asking process
//! (`SO_PEERCRED`); its executable and working folder come from `/proc`.

use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;

use crate::os::{Peer, ServeKeys};

pub fn socket(profile: &str) -> PathBuf {
    super::runtime_dir().join(format!("handoff-{profile}-keys.sock"))
}

/// Serve programs asking for keys, each connection on its own thread.
pub fn listen_for_keys(profile: &str, serve: ServeKeys) -> Result<(), String> {
    let path = socket(profile);
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path).map_err(|e| e.to_string())?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    thread::Builder::new()
        .name("keys".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let Some(peer) = peer(&stream) else { continue };
                let serve = serve.clone();
                let _ = thread::Builder::new().name("key-ask".into()).spawn(move || serve(peer, Box::new(stream)));
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// The connected process, if it runs as this user.
fn peer(stream: &UnixStream) -> Option<Peer> {
    // SAFETY: zeroed ucred is valid; getsockopt writes at most `len` bytes.
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let ok = unsafe {
        libc::getsockopt(stream.as_raw_fd(), libc::SOL_SOCKET, libc::SO_PEERCRED, (&raw mut cred).cast(), &mut len)
    } == 0;
    // SAFETY: getuid cannot fail.
    if !ok || cred.uid != unsafe { libc::getuid() } {
        return None;
    }
    let proc = PathBuf::from(format!("/proc/{}", cred.pid));
    let program = fs::read_link(proc.join("exe")).ok()?;
    Some(Peer { program, folder: fs::read_link(proc.join("cwd")).ok() })
}
