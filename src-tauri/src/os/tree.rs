//! Who started a request: the rules, once for every system. Each OS only
//! answers questions about one process (its parent, names, arguments,
//! environment, start time) through [`ProcessTable`]; walking up the tree and
//! deciding what counts as an agent or an agent's session happens here.

use std::path::PathBuf;

use super::{agent_named, AgentProcess};

/// How far up the tree any walk goes, so a loop in bad data can't hang it.
const MAX_DEPTH: usize = 64;

/// What the OS can say about one process. Anything it won't say is `None`
/// or empty, and the rules treat that as "not this one".
pub trait ProcessTable {
    /// The process that started `pid`, or `None` at the top of the tree
    /// (init, launchd, a pid of 0) or when it's gone.
    fn parent(&self, pid: u32) -> Option<u32>;
    /// The short name the OS gives the process (`comm`, Activity Monitor's).
    fn short_name(&self, pid: u32) -> Option<String>;
    /// Its executable.
    fn image(&self, pid: u32) -> Option<PathBuf>;
    /// Its arguments, the program first.
    fn args(&self, pid: u32) -> Vec<String>;
    /// Whether its environment holds exactly `entry` (`NAME=value`).
    fn environ_has(&self, pid: u32, entry: &[u8]) -> bool;
    /// When it started, in any unit that only grows; tells a reused pid apart.
    fn started(&self, pid: u32) -> Option<u64>;
}

/// The AI coding agent among `pid` and the processes that started it: each
/// one's name, executable and first two arguments are checked, and Claude
/// Code's `CLAUDECODE` marker in the environment. A renamed agent isn't
/// found, so this is a warning; binding approvals to the command is what holds.
pub fn agent(table: &impl ProcessTable, pid: u32) -> Option<String> {
    if table.environ_has(pid, b"CLAUDECODE=1") {
        return Some("Claude Code".into());
    }
    let mut pid = pid;
    for _ in 0..MAX_DEPTH {
        let image = table.image(pid).map(|p| p.to_string_lossy().into_owned());
        let names = [table.short_name(pid), image].into_iter().flatten().chain(table.args(pid).into_iter().take(2));
        if let Some(agent) = names.filter(|n| !n.is_empty()).find_map(|n| agent_named(&n)) {
            return Some(agent);
        }
        pid = table.parent(pid)?;
    }
    None
}

/// The dedicated interactive agent CLI process (`claude`, or `codex` alone or
/// as `codex exec`) among `pid` and the processes that started it, if it's
/// the `expected` agent: the scope a session grant is bound to. Each process
/// must be older than the one it started, and its start time tells a reused
/// pid apart. Hosts that can run several chats in one process, and any other
/// mode, can't establish a session.
#[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
pub fn agent_session(table: &impl ProcessTable, pid: u32, expected: &str) -> Option<AgentProcess> {
    let mut pid = pid;
    let mut child_birth = u64::MAX;
    for _ in 0..MAX_DEPTH {
        let birth = table.started(pid)?;
        if birth > child_birth {
            return None;
        }
        let image = table.image(pid)?;
        // Native installers start a version-named file through a `claude`
        // link, so the short name is checked as well as the file.
        let file = image.file_name()?.to_str()?.to_string();
        let short = table.short_name(pid).unwrap_or_default();
        if let Some((agent, name)) = [short, file].into_iter().find_map(|n| agent_named(&n).map(|a| (a, n))) {
            if agent != expected || !(name == "claude" || name == "codex") {
                return None;
            }
            let argv = table.args(pid);
            let dedicated = argv.len() == 1 || (name == "codex" && argv.get(1).is_some_and(|a| a == "exec"));
            return dedicated.then_some(AgentProcess { pid, created: birth, image });
        }
        child_birth = birth;
        pid = table.parent(pid)?;
    }
    None
}

/// Whether `pid` is `ancestor` or was started, directly or not, by it.
pub fn descends_from(table: &impl ProcessTable, pid: u32, ancestor: u32) -> bool {
    let mut pid = pid;
    for _ in 0..MAX_DEPTH {
        if pid == ancestor {
            return true;
        }
        match table.parent(pid) {
            Some(p) => pid = p,
            None => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A made-up process tree: pid → (parent, name, args, start).
    struct Fake(HashMap<u32, (u32, &'static str, Vec<&'static str>, u64)>);

    impl ProcessTable for Fake {
        fn parent(&self, pid: u32) -> Option<u32> {
            self.0.get(&pid).map(|p| p.0).filter(|&p| p > 1)
        }
        fn short_name(&self, pid: u32) -> Option<String> {
            self.0.get(&pid).map(|p| p.1.to_string())
        }
        fn image(&self, pid: u32) -> Option<PathBuf> {
            self.0.get(&pid).map(|p| PathBuf::from("/usr/bin").join(p.1))
        }
        fn args(&self, pid: u32) -> Vec<String> {
            self.0.get(&pid).map(|p| p.2.iter().map(|a| a.to_string()).collect()).unwrap_or_default()
        }
        fn environ_has(&self, _: u32, _: &[u8]) -> bool {
            false
        }
        fn started(&self, pid: u32) -> Option<u64> {
            self.0.get(&pid).map(|p| p.3)
        }
    }

    fn tree() -> Fake {
        Fake(HashMap::from([
            (10, (1, "zsh", vec!["zsh"], 100)),
            (20, (10, "claude", vec!["claude"], 200)),
            (30, (20, "bash", vec!["bash", "-c", "npm test"], 300)),
            (40, (30, "node", vec!["node", "test.js"], 400)),
            // A pid reused after its "parent" started: younger than its child.
            (50, (60, "node", vec!["node"], 500)),
            (60, (10, "claude", vec!["claude"], 900)),
            (70, (10, "codex", vec!["codex", "app-server"], 150)),
            (71, (70, "node", vec!["node"], 160)),
        ]))
    }

    #[test]
    fn the_agent_and_its_session_are_found_up_the_tree() {
        let t = tree();
        assert_eq!(agent(&t, 40).as_deref(), Some("Claude Code"));
        assert_eq!(agent(&t, 10), None);
        let session = agent_session(&t, 40, "Claude Code").unwrap();
        assert_eq!((session.pid, session.created), (20, 200));
        assert!(agent_session(&t, 40, "Codex").is_none(), "only the expected agent");
        assert!(agent_session(&t, 50, "Claude Code").is_none(), "a parent younger than its child is a reused pid");
        assert!(agent_session(&t, 71, "Codex").is_none(), "codex app-server isn't a dedicated session");
        assert!(descends_from(&t, 40, 20) && descends_from(&t, 40, 40) && !descends_from(&t, 40, 70));
    }
}
