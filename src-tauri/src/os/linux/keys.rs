//! Programs ask for kept keys over a Unix socket in the runtime directory,
//! which only this user can enter. The kernel reports the asking process
//! (`SO_PEERCRED`); its executable and working folder come from `/proc`.

use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::thread;

use crate::os::{Peer, ServeKeys};

pub fn socket(profile: &str) -> PathBuf {
    super::runtime_dir().join(format!("handoff-{profile}-keys.sock"))
}

/// Serve programs asking for keys, each connection on its own thread.
pub fn listen_for_keys(profile: &str, serve: ServeKeys) -> Result<(), String> {
    super::serve_socket(&socket(profile), "keys", move |stream| {
        let Some(peer) = peer(&stream) else { return };
        let serve = serve.clone();
        let _ = thread::Builder::new().name("key-ask".into()).spawn(move || serve(peer, Box::new(stream)));
    })
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
    let argv = args(cred.pid);
    let command = super::super::display_command(argv.iter().map(String::as_str));
    let args = argv.into_iter().skip(1).collect();
    Some(Peer { pid: cred.pid as u32, program, folder: fs::read_link(proc.join("cwd")).ok(), command, args, agent: agent(cred.pid) })
}

fn args(pid: i32) -> Vec<String> {
    let cmdline = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    String::from_utf8_lossy(&cmdline).split('\0').filter(|a| !a.is_empty()).map(String::from).collect()
}

/// The AI coding agent among `pid` and the processes that started it: each
/// one's name, executable and first two arguments are checked, and Claude
/// Code's `CLAUDECODE` marker in the environment. A renamed agent isn't
/// found, so this is a warning; binding approvals to the command is what holds.
pub fn agent(pid: i32) -> Option<String> {
    let environ = fs::read(format!("/proc/{pid}/environ")).unwrap_or_default();
    if environ.split(|&b| b == 0).any(|v| v == b"CLAUDECODE=1") {
        return Some("Claude Code".into());
    }
    let mut pid = pid;
    for _ in 0..64 {
        let proc = PathBuf::from(format!("/proc/{pid}"));
        let comm = fs::read_to_string(proc.join("comm")).unwrap_or_default();
        let exe = fs::read_link(proc.join("exe")).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let names = [comm.trim_end().to_string(), exe].into_iter().chain(args(pid).into_iter().take(2));
        if let Some(agent) = names.filter(|n| !n.is_empty()).find_map(|n| super::super::agent_named(&n)) {
            return Some(agent);
        }
        // `pid (comm) state ppid …`; comm can hold spaces and parentheses.
        let stat = fs::read_to_string(proc.join("stat")).ok()?;
        let ppid: i32 = stat.rsplit_once(')')?.1.split_whitespace().nth(1)?.parse().ok()?;
        if ppid <= 1 {
            return None;
        }
        pid = ppid;
    }
    None
}

/// Whether `pid` is `ancestor` or was started, directly or not, by it.
pub fn descends_from(pid: u32, ancestor: u32) -> bool {
    let mut pid = pid;
    for _ in 0..64 {
        if pid == ancestor {
            return true;
        }
        let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else { return false };
        let Some(ppid) = stat.rsplit_once(')').and_then(|(_, rest)| rest.split_whitespace().nth(1)?.parse().ok()) else { return false };
        if ppid <= 1 {
            return false;
        }
        pid = ppid;
    }
    false
}

/// Connect to a running KeyHandoff's key socket, as `keyhandoff run` does.
pub fn connect_keys(profile: &str) -> std::io::Result<Box<dyn crate::os::KeyStream>> {
    Ok(Box::new(UnixStream::connect(socket(profile))?))
}
