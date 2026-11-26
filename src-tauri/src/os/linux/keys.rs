//! Programs ask for kept keys over a Unix socket in the runtime directory,
//! which only this user can enter. The kernel reports the asking process
//! (`SO_PEERCRED`); its executable and working folder come from `/proc`.

use std::fs;
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
    Some(Peer { pid: cred.pid as u32, program, folder: fs::read_link(proc.join("cwd")).ok(), command, args, agent: None })
}

fn args(pid: i32) -> Vec<String> {
    let cmdline = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    String::from_utf8_lossy(&cmdline).split('\0').filter(|a| !a.is_empty()).map(String::from).collect()
}

/// What `/proc` says about one process, for the shared rules in `os::tree`.
struct Table;

impl super::super::tree::ProcessTable for Table {
    fn parent(&self, pid: u32) -> Option<u32> {
        stat_field(pid as i32, 1).filter(|&p| p > 1 && p != pid as i64).map(|p| p as u32)
    }
    fn short_name(&self, pid: u32) -> Option<String> {
        fs::read_to_string(format!("/proc/{pid}/comm")).ok().map(|c| c.trim_end().to_string())
    }
    fn image(&self, pid: u32) -> Option<PathBuf> {
        fs::read_link(format!("/proc/{pid}/exe")).ok()
    }
    fn args(&self, pid: u32) -> Vec<String> {
        args(pid as i32)
    }
    fn environ_has(&self, pid: u32, entry: &[u8]) -> bool {
        fs::read(format!("/proc/{pid}/environ")).unwrap_or_default().split(|&b| b == 0).any(|v| v == entry)
    }
    fn started(&self, pid: u32) -> Option<u64> {
        // `starttime` is field 22 of stat, 19 after the state: clock ticks since boot.
        u64::try_from(stat_field(pid as i32, 19)?).ok()
    }
}

pub fn agent(pid: u32) -> Option<String> {
    super::super::tree::agent(&Table, pid)
}

impl Peer {
    /// What started this request when no person did: a cron or at job, or a
    /// systemd service or timer. Such a request needs an answer every time
    /// and can't use or create a lasting grant, as on Windows (FIX-08). Read
    /// from `/proc`: the names of the processes that started it, its cgroup
    /// (which systemd unit it runs in), and whether it has a terminal. A job
    /// that reparents itself away from cron still keeps its cgroup; one that
    /// also moves itself into the desktop session isn't caught, so this is a
    /// guard, and binding approvals to the command is what holds.
    pub fn unattended_origin(&self) -> Option<&'static str> {
        if self.pid == 0 {
            return None;
        }
        let pid = self.pid as i32;
        let cgroup = fs::read_to_string(format!("/proc/{pid}/cgroup")).unwrap_or_default();
        let tty = stat_field(pid, 4).is_some_and(|tty| tty != 0);
        unattended_origin(&ancestry(pid), &cgroup, tty)
    }
}

/// Field `n` after the command name in `/proc/<pid>/stat`, counting the
/// state as 0: 1 is the parent, 4 the controlling terminal.
fn stat_field(pid: i32, n: usize) -> Option<i64> {
    // `pid (comm) state ppid …`; comm can hold spaces and parentheses.
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?.1.split_whitespace().nth(n)?.parse().ok()
}

/// The command names of `pid` and the processes that started it.
fn ancestry(mut pid: i32) -> Vec<String> {
    let mut names = Vec::new();
    for _ in 0..64 {
        let Ok(comm) = fs::read_to_string(format!("/proc/{pid}/comm")) else { break };
        names.push(comm.trim_end().to_string());
        match stat_field(pid, 1) {
            Some(ppid) if ppid > 1 && ppid != pid as i64 => pid = ppid as i32,
            _ => break,
        }
    }
    names
}

fn unattended_origin(ancestry: &[String], cgroup: &str, tty: bool) -> Option<&'static str> {
    for name in ancestry {
        let name = name.to_ascii_lowercase();
        if ["cron", "crond", "anacron", "fcron"].contains(&name.as_str()) {
            return Some("a cron job");
        }
        if name == "atd" {
            return Some("an at job");
        }
    }
    // The unified hierarchy's line, `0::/…`; its last part is the unit.
    let path = cgroup.lines().find_map(|l| l.strip_prefix("0::")).unwrap_or("");
    if path.starts_with("/system.slice/") {
        return Some("a system service");
    }
    let unit = path.rsplit('/').find(|part| !part.is_empty()).unwrap_or("");
    // `systemd-run --user` names its services run-….service; with --pty it
    // has a terminal, but it starts outside the caller's process tree and
    // environment, so an agent could use it to look like a person.
    if path.contains("/user@") && unit.starts_with("run-") && unit.ends_with(".service") {
        return Some("a systemd-run service");
    }
    // A person at a terminal, even one a user service hosts (older
    // gnome-terminal runs its shells inside gnome-terminal-server.service).
    if tty {
        return None;
    }
    // Desktops launch apps as app-….scope, or app-….service on KDE; any
    // other service under the user's manager is a user service or timer.
    if path.contains("/user@") && unit.ends_with(".service") && !unit.starts_with("app-") && !unit.starts_with("user@") {
        return Some("a systemd user service or timer");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    fn names(n: &[&str]) -> Vec<String> {
        n.iter().map(|s| s.to_string()).collect()
    }

    fn user_unit(unit: &str) -> String {
        format!("0::/user.slice/user-1000.slice/user@1000.service/app.slice/{unit}\n")
    }

    #[test]
    fn unattended_launches_are_distinguished_from_a_person() {
        // cron and at, wherever the job ended up.
        assert_eq!(unattended_origin(&names(&["python3", "sh", "CRON", "cron"]), "0::/system.slice/cron.service\n", false), Some("a cron job"));
        assert_eq!(unattended_origin(&names(&["python3", "sh", "atd"]), "0::/system.slice/atd.service\n", false), Some("an at job"));
        // A job that left cron's process tree keeps its cgroup.
        assert_eq!(unattended_origin(&names(&["python3"]), "0::/system.slice/cron.service\n", false), Some("a system service"));
        // System services and timers, even with a terminal.
        assert_eq!(unattended_origin(&names(&["python3"]), "0::/system.slice/report.service\n", true), Some("a system service"));
        // User services and timers (systemd-run --user, a .timer's service).
        assert_eq!(unattended_origin(&names(&["python3"]), &user_unit("report.service"), false), Some("a systemd user service or timer"));
        // systemd-run --user, with or without --pty: outside the caller's
        // tree and environment, so a terminal doesn't make it a person.
        assert_eq!(unattended_origin(&names(&["python3"]), &user_unit("run-u42.service"), true), Some("a systemd-run service"));
        assert_eq!(unattended_origin(&names(&["python3"]), &user_unit("run-p1234-i1235.service"), false), Some("a systemd-run service"));
        // A timer's service with no terminal, as financeboy-pull.timer runs.
        assert_eq!(unattended_origin(&names(&["node", "keyhandoff"]), &user_unit("financeboy-pull.service"), false), Some("a systemd user service or timer"));

        // People: a terminal, apps the desktop launched (GNOME scopes, KDE
        // services), a shell hosted by gnome-terminal-server.service, SSH,
        // and cgroup v1 only, where just the process names count.
        let terminal = user_unit("app-org.gnome.Terminal.slice/vte-spawn-1.scope");
        assert_eq!(unattended_origin(&names(&["python3", "bash", "gnome-terminal-"]), &terminal, true), None);
        assert_eq!(unattended_origin(&names(&["python3", "code"]), &user_unit("app-code@1a2b.service"), false), None);
        assert_eq!(unattended_origin(&names(&["python3", "code"]), &user_unit("app-gnome-code-4242.scope"), false), None);
        assert_eq!(unattended_origin(&names(&["python3", "bash"]), &user_unit("gnome-terminal-server.service"), true), None);
        assert_eq!(unattended_origin(&names(&["python3", "bash", "sshd"]), "0::/user.slice/user-1000.slice/session-4.scope\n", true), None);
        assert_eq!(unattended_origin(&names(&["python3", "bash"]), "", false), None);
    }

    /// A real process tree read from `/proc`, as a request's peer is: a
    /// program started by a process named `cron` is a cron job.
    #[test]
    fn a_program_started_by_cron_is_unattended() {
        let dir = std::env::temp_dir().join(format!("kh-cron-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let cron = dir.join("cron");
        fs::copy("/bin/sh", &cron).unwrap();
        // A copy of sh named cron starts the "job" and prints its pid.
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
        // This test process, started by cargo, isn't a cron job.
        assert_ne!(me, Some("a cron job"));
    }
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
