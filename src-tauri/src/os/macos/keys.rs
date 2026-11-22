//! Programs ask for kept keys over a Unix socket in the per-user temporary
//! folder, which only this user can enter. The kernel reports the asking
//! process (`LOCAL_PEERPID`, and its user with `getpeereid`); its
//! executable, working folder and arguments come from libproc and `sysctl`.

use std::ffi::CStr;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::thread;

use crate::os::{AgentProcess, Peer, ServeKeys};

pub fn socket(profile: &str) -> PathBuf {
    super::runtime_dir().join(format!("handoff-{profile}-keys.sock"))
}

/// Serve programs asking for keys, each connection on its own thread.
pub fn listen_for_keys(profile: &str, serve: ServeKeys) -> Result<(), String> {
    let connections = crate::os::KeyConnections::default();
    super::serve_socket(&socket(profile), "keys", move |stream| {
        let Some(permit) = connections.enter() else { return };
        let Some(peer) = peer(&stream) else { return };
        let serve = serve.clone();
        let _ = thread::Builder::new().name("key-ask".into()).spawn(move || {
            let _permit = permit;
            serve(peer, Box::new(stream));
        });
    })
}

/// The connected process, if it runs as this user.
fn peer(stream: &UnixStream) -> Option<Peer> {
    let fd = stream.as_raw_fd();
    let (mut uid, mut gid) = (0, 0);
    // SAFETY: getpeereid writes two ids for a connected Unix socket.
    if unsafe { libc::getpeereid(fd, &mut uid, &mut gid) } != 0 || uid != unsafe { libc::getuid() } {
        return None;
    }
    let mut pid: libc::pid_t = 0;
    let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    // SAFETY: LOCAL_PEERPID writes one pid_t, at most `len` bytes.
    let ok = unsafe { libc::getsockopt(fd, libc::SOL_LOCAL, libc::LOCAL_PEERPID, (&raw mut pid).cast(), &mut len) } == 0;
    if !ok || pid <= 0 {
        return None;
    }
    let program = path(pid)?;
    let (argv, _) = procargs(pid);
    let command = super::super::display_command(argv.iter().map(String::as_str));
    let args = argv.into_iter().skip(1).collect();
    Some(Peer { pid: pid as u32, program, folder: cwd(pid), command, args, agent: None })
}

// --- process details from libproc and sysctl ---

/// Every process id the system lists.
pub(super) fn all_pids() -> Vec<i32> {
    // SAFETY: a null buffer asks for the count; the second call fills at most `cap` bytes.
    let n = unsafe { libc::proc_listallpids(std::ptr::null_mut(), 0) };
    if n <= 0 {
        return Vec::new();
    }
    let mut pids = vec![0i32; n as usize + 64];
    let cap = (pids.len() * std::mem::size_of::<i32>()) as libc::c_int;
    let got = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), cap) };
    pids.truncate(got.max(0) as usize);
    pids.retain(|&p| p > 0);
    pids
}

/// A process's short name, as Activity Monitor shows it (up to 32 bytes).
pub(super) fn name(pid: i32) -> Option<String> {
    let mut buf = [0u8; 2 * libc::MAXCOMLEN + 1];
    // SAFETY: proc_name writes at most `buf.len()` bytes.
    let n = unsafe { libc::proc_name(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    (n > 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
}

/// The process's executable.
pub(super) fn path(pid: i32) -> Option<PathBuf> {
    let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: proc_pidpath writes at most `buf.len()` bytes.
    let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    if n <= 0 {
        return None;
    }
    buf.truncate(n as usize);
    use std::os::unix::ffi::OsStringExt;
    Some(PathBuf::from(std::ffi::OsString::from_vec(buf)))
}

fn bsdinfo(pid: i32) -> Option<libc::proc_bsdinfo> {
    // SAFETY: zeroed is valid for this plain C struct; the call fills it.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let n = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, (&raw mut info).cast(), size) };
    (n == size).then_some(info)
}

/// The parent's pid, or `None` at launchd (pid 1) or when the process is gone.
fn parent(pid: i32) -> Option<i32> {
    let ppid = bsdinfo(pid)?.pbi_ppid as i32;
    (ppid > 1 && ppid != pid).then_some(ppid)
}

/// When the process started, in microseconds since the epoch.
fn started(pid: i32) -> Option<u64> {
    let info = bsdinfo(pid)?;
    Some(info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec)
}

/// The process's working folder.
fn cwd(pid: i32) -> Option<PathBuf> {
    // SAFETY: zeroed is valid for this plain C struct; the call fills it.
    let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
    let n = unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDVNODEPATHINFO, 0, (&raw mut info).cast(), size) };
    if n != size {
        return None;
    }
    // vip_path is a MAXPATHLEN C string, split into rows for old rustc.
    // SAFETY: the kernel writes a NUL-terminated path within the array.
    let raw = unsafe { CStr::from_ptr(info.pvi_cdir.vip_path.as_ptr().cast()) };
    let bytes = raw.to_bytes();
    if bytes.is_empty() {
        return None;
    }
    use std::os::unix::ffi::OsStrExt;
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
}

/// The process's arguments and environment, from `KERN_PROCARGS2`: argc,
/// the executable path, padding, argv, then the environment. Readable for
/// this user's own processes.
pub(super) fn procargs(pid: i32) -> (Vec<String>, Vec<Vec<u8>>) {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size: libc::size_t = 0;
    // SAFETY: first call sizes the buffer, second fills at most `size` bytes.
    let sized = unsafe { libc::sysctl(mib.as_mut_ptr(), 3, std::ptr::null_mut(), &mut size, std::ptr::null_mut(), 0) };
    if sized != 0 || size < 4 {
        return (Vec::new(), Vec::new());
    }
    let mut buf = vec![0u8; size];
    let filled = unsafe { libc::sysctl(mib.as_mut_ptr(), 3, buf.as_mut_ptr().cast(), &mut size, std::ptr::null_mut(), 0) };
    if filled != 0 {
        return (Vec::new(), Vec::new());
    }
    buf.truncate(size);
    parse_procargs(&buf)
}

fn parse_procargs(buf: &[u8]) -> (Vec<String>, Vec<Vec<u8>>) {
    let Some(argc) = buf.get(..4).map(|b| i32::from_ne_bytes([b[0], b[1], b[2], b[3]])) else { return (Vec::new(), Vec::new()) };
    let rest = &buf[4..];
    // Skip the executable path and the NULs that pad it.
    let Some(end) = rest.iter().position(|&b| b == 0) else { return (Vec::new(), Vec::new()) };
    let mut i = end;
    while i < rest.len() && rest[i] == 0 {
        i += 1;
    }
    let mut parts = rest[i..].split(|&b| b == 0);
    let argv: Vec<String> = parts.by_ref().take(argc.max(0) as usize).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
    let env = parts.take_while(|v| !v.is_empty()).map(<[u8]>::to_vec).collect();
    (argv, env)
}

/// What macOS says about one process, for the shared rules in `os::tree`.
struct Table;

impl super::super::tree::ProcessTable for Table {
    fn parent(&self, pid: u32) -> Option<u32> {
        parent(pid as i32).map(|p| p as u32)
    }
    fn short_name(&self, pid: u32) -> Option<String> {
        name(pid as i32)
    }
    fn image(&self, pid: u32) -> Option<PathBuf> {
        path(pid as i32)
    }
    fn args(&self, pid: u32) -> Vec<String> {
        procargs(pid as i32).0
    }
    fn environ_has(&self, pid: u32, entry: &[u8]) -> bool {
        procargs(pid as i32).1.iter().any(|v| v == entry)
    }
    fn started(&self, pid: u32) -> Option<u64> {
        started(pid as i32)
    }
}

pub fn agent(pid: u32) -> Option<String> {
    super::super::tree::agent(&Table, pid)
}

impl Peer {
    /// What started this request when no person did: a cron or at job, or a
    /// launchd agent or daemon. Such a request needs an answer every time and
    /// can't use or create a lasting grant, as on Windows and Linux. Read
    /// from the process tree: a terminal means a person; otherwise, the
    /// process launchd started is checked, and an app bundle (Terminal, an
    /// editor) means a person, while a bare program is a launchd job. A job
    /// that starts itself through an app bundle isn't caught, so this is a
    /// guard, and binding approvals to the command is what holds.
    pub fn unattended_origin(&self) -> Option<&'static str> {
        if self.pid == 0 {
            return None;
        }
        let pid = self.pid as i32;
        let tty = bsdinfo(pid).is_some_and(|i| i.e_tdev != 0 && i.e_tdev != u32::MAX);
        let mut chain = Vec::new();
        let mut at = pid;
        for _ in 0..64 {
            chain.push((name(at).unwrap_or_default(), path(at).unwrap_or_default()));
            match parent(at) {
                Some(p) => at = p,
                None => break,
            }
        }
        unattended_origin(&chain, tty)
    }
}

/// `chain` is (name, executable) from the asking process up to the one
/// launchd started.
fn unattended_origin(chain: &[(String, PathBuf)], tty: bool) -> Option<&'static str> {
    for (name, _) in chain {
        let name = name.to_ascii_lowercase();
        if name == "cron" {
            return Some("a cron job");
        }
        if name == "atrun" {
            return Some("an at job");
        }
    }
    if tty {
        return None;
    }
    let (_, top) = chain.last()?;
    let top = top.to_string_lossy();
    // Apps the person opened (Terminal, iTerm, VS Code, Cursor) and the
    // helpers inside their bundles; sshd and login sessions have a terminal.
    if top.contains(".app/Contents/") || top.ends_with("/sshd") || top.ends_with("/login") {
        return None;
    }
    Some("a launchd agent or daemon")
}

pub fn agent_session(pid: u32, expected: &str) -> Option<AgentProcess> {
    super::super::tree::agent_session(&Table, pid, expected)
}

pub fn descends_from(pid: u32, ancestor: u32) -> bool {
    super::super::tree::descends_from(&Table, pid, ancestor)
}

/// Connect to a running KeyHandoff's key socket, as `keyhandoff run` does.
pub fn connect_keys(profile: &str) -> std::io::Result<Box<dyn crate::os::KeyStream>> {
    Ok(Box::new(UnixStream::connect(socket(profile))?))
}

impl crate::os::KeyStream for UnixStream {
    fn set_read_timeout(&mut self, timeout: Option<std::time::Duration>) -> std::io::Result<()> {
        UnixStream::set_read_timeout(self, timeout)
    }

    fn set_write_timeout(&mut self, timeout: Option<std::time::Duration>) -> std::io::Result<()> {
        UnixStream::set_write_timeout(self, timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::{Command, Stdio};

    fn chain(n: &[(&str, &str)]) -> Vec<(String, PathBuf)> {
        n.iter().map(|(a, b)| (a.to_string(), PathBuf::from(b))).collect()
    }

    #[test]
    fn unattended_launches_are_distinguished_from_a_person() {
        assert_eq!(unattended_origin(&chain(&[("python3", "/usr/bin/python3"), ("sh", "/bin/sh"), ("cron", "/usr/sbin/cron")]), false), Some("a cron job"));
        assert_eq!(unattended_origin(&chain(&[("python3", "/usr/bin/python3"), ("atrun", "/usr/libexec/atrun")]), false), Some("an at job"));
        // A LaunchAgent's program, started straight by launchd.
        assert_eq!(unattended_origin(&chain(&[("python3", "/usr/bin/python3"), ("report", "/Users/a/bin/report")]), false), Some("a launchd agent or daemon"));
        // Even cron with a terminal counts as cron.
        assert_eq!(unattended_origin(&chain(&[("python3", ""), ("cron", "/usr/sbin/cron")]), true), Some("a cron job"));

        // People: a terminal, an app's integrated terminal or helper, ssh.
        assert_eq!(unattended_origin(&chain(&[("python3", ""), ("zsh", "/bin/zsh"), ("login", "/usr/bin/login")]), true), None);
        let code = "/Applications/Visual Studio Code.app/Contents/MacOS/Electron";
        assert_eq!(unattended_origin(&chain(&[("python3", ""), ("Code Helper", code)]), false), None);
        assert_eq!(unattended_origin(&chain(&[("python3", ""), ("sshd", "/usr/sbin/sshd")]), false), None);
    }

    /// A real process tree, as a request's peer is read: a program started
    /// by a process named `cron` is a cron job, and this test isn't.
    #[test]
    fn a_program_started_by_cron_is_unattended() {
        let dir = std::env::temp_dir().join(format!("kh-cron-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let cron = dir.join("cron");
        // Not /bin/sh: on macOS it starts bash or zsh as another process.
        fs::copy("/bin/bash", &cron).unwrap();
        let mut parent = Command::new(&cron).args(["-c", "sleep 30 & echo $!; wait"]).stdout(Stdio::piped()).spawn().unwrap();
        let mut line = String::new();
        std::io::BufRead::read_line(&mut std::io::BufReader::new(parent.stdout.take().unwrap()), &mut line).unwrap();
        let job: u32 = line.trim().parse().unwrap();
        let peer = |pid| Peer { pid, program: PathBuf::from("/bin/sleep"), folder: None, command: None, args: vec![], agent: None };
        let origin = peer(job).unattended_origin();
        let me = peer(std::process::id()).unattended_origin();
        let _ = Command::new("kill").arg(job.to_string()).status();
        let _ = parent.wait();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(origin, Some("a cron job"));
        assert_ne!(me, Some("a cron job"));
    }

    /// The socket names the real asking process: its executable, folder and
    /// arguments, and a child is seen to descend from its parent.
    #[test]
    fn the_socket_names_the_asking_program() {
        let profile = format!("test-peer-{}", std::process::id());
        let (tx, rx) = std::sync::mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        listen_for_keys(&profile, std::sync::Arc::new(move |peer, _stream| tx.lock().unwrap().send(peer).unwrap())).unwrap();
        let folder = std::env::temp_dir().join(format!("kh-peer-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let folder = folder.canonicalize().unwrap();
        let sock = socket(&profile);
        // `nc -U` connects as its own process from `folder`.
        let mut nc = Command::new("/usr/bin/nc").args(["-U", sock.to_str().unwrap()]).current_dir(&folder).stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
        let peer = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        let _ = nc.kill();
        let _ = nc.wait();
        let _ = fs::remove_file(&sock);
        let _ = fs::remove_dir_all(&folder);
        assert_eq!(peer.pid, nc.id());
        assert_eq!(peer.program, PathBuf::from("/usr/bin/nc"));
        assert_eq!(peer.folder.as_deref(), Some(folder.as_path()));
        assert_eq!(peer.args, vec!["-U".to_string(), sock.to_str().unwrap().to_string()]);
        assert_eq!(peer.command.as_deref(), Some(format!("nc -U {}", sock.display()).as_str()));
    }

    #[test]
    fn process_details_come_from_the_kernel() {
        let me = std::process::id();
        // A copy: macOS hides the environment of programs SIP protects.
        let dir = std::env::temp_dir().join(format!("kh-probe-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let sleep = dir.join("sleep");
        fs::copy("/bin/sleep", &sleep).unwrap();
        let mut child = Command::new(&sleep).arg("5").env("KH_PROBE", "1").spawn().unwrap();
        let pid = child.id() as i32;
        std::thread::sleep(std::time::Duration::from_millis(100));
        let (argv, env) = procargs(pid);
        assert_eq!(argv, vec![sleep.display().to_string(), "5".to_string()]);
        assert!(env.iter().any(|v| v == b"KH_PROBE=1"));
        assert!(descends_from(pid as u32, me));
        assert!(!descends_from(me, pid as u32));
        assert_eq!(name(pid).as_deref(), Some("sleep"));
        assert!(started(pid).unwrap() >= started(me as i32).unwrap());
        let _ = child.kill();
        let _ = child.wait();
        let _ = fs::remove_dir_all(&dir);
    }
}
