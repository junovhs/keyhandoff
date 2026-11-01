//! Per-OS backends behind one small interface: the clipboard (Safe Copy
//! capture, putting a file on the clipboard, reading a paste), the master key
//! the OS protects for each profile, dragging a file out of a window, and the
//! local socket or pipe programs ask for kept keys over.
//!
//! Linux (X11) was built first, then Windows (clipboard history, DPAPI); each
//! is a folder with the same functions.

use std::io::{Read, Write};
use std::path::PathBuf;

use keyhandoff_seal::Secret;
use zeroize::Zeroizing;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(not(any(target_os = "linux", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", windows)))]
pub use unsupported::*;

/// How a Safe Copy capture ended.
pub enum Captured {
    /// The key, and a note for the user if wiping it fell short (for
    /// example, Windows clipboard history had to be cleared).
    Caught(Secret, Option<String>),
    TimedOut,
    Cancelled,
    /// Linux only so far; Windows reports setup errors from `arm_capture`.
    #[cfg_attr(windows, allow(dead_code))]
    Failed(String),
}

/// What a paste found on the clipboard.
pub enum Clip {
    Files(Vec<PathBuf>),
    Text(Zeroizing<String>),
    Empty,
}

/// The display names of known clipboard apps among running process names,
/// sorted and without repeats. `table` maps a process name to a display name.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
fn known_apps(running: impl Iterator<Item = String>, table: &[(&str, &str)]) -> Vec<String> {
    let mut found: Vec<String> =
        running.filter_map(|n| table.iter().find(|(process, _)| *process == n).map(|(_, name)| name.to_string())).collect();
    found.sort();
    found.dedup();
    found
}

/// A profile's 32-byte master key, held by the OS and zeroized on drop.
pub struct MasterKey(pub(crate) Zeroizing<[u8; 32]>);

impl MasterKey {
    #[cfg(test)]
    pub fn for_tests() -> Self {
        use rand_core::{OsRng, RngCore};
        let mut key = Zeroizing::new([0u8; 32]);
        OsRng.fill_bytes(&mut key[..]);
        Self(key)
    }
}

/// A local program asking for kept keys, as the OS reports it: its
/// executable and its working folder (`None` when the OS won't say).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    pub program: PathBuf,
    pub folder: Option<PathBuf>,
    /// `python3 report.py`: shown so you recognise the script.
    pub command: Option<String>,
    /// The arguments after the program. An approval is bound to them and to
    /// the files they name ([`Peer::binding`]). A program reports its own
    /// arguments, so for a loader this binds what it claims to run; under
    /// `keyhandoff run` it's what really runs.
    pub args: Vec<String>,
    /// The AI coding agent among the processes that started it, if one is.
    pub agent: Option<String>,
}

impl Peer {
    /// What an approval is bound to: the program, its arguments, and the
    /// contents of every file the arguments name (a script, a config), so an
    /// edited script, or the same interpreter running other code, asks again.
    pub fn binding(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        let mut part = |bytes: &[u8]| {
            h.update((bytes.len() as u64).to_le_bytes());
            h.update(bytes);
        };
        part(self.program.as_os_str().as_encoded_bytes());
        for arg in &self.args {
            part(arg.as_bytes());
            let path = match &self.folder {
                Some(folder) => folder.join(arg),
                None => PathBuf::from(arg),
            };
            match file_hash(&path) {
                Some(hash) => part(&hash),
                None => part(b""),
            }
        }
        h.finalize().into()
    }
}

/// The SHA-256 of a regular file's contents, if `path` is one.
pub fn file_hash(path: &std::path::Path) -> Option<[u8; 32]> {
    use sha2::{Digest, Sha256};
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let mut h = Sha256::new();
    std::io::copy(&mut file, &mut h).ok()?;
    Some(h.finalize().into())
}

/// AI coding agents by process name (lowercase, without `.exe`). Claude Code
/// also runs as `node …/claude-code/cli.js`, so a name containing the second
/// column matches too.
const AGENTS: &[(&str, &str, &str)] = &[
    ("claude", "claude-code", "Claude Code"),
    ("cursor", "cursor-agent", "Cursor"),
    ("codex", "@openai/codex", "Codex"),
    ("copilot", "github-copilot", "GitHub Copilot"),
    ("aider", "aider-chat", "Aider"),
    ("windsurf", "windsurf", "Windsurf"),
    ("gemini", "gemini-cli", "Gemini CLI"),
];

/// The agent a process name or path names, if any: matched on the file
/// name, so `claude-desktop` or a folder called `codex` doesn't count.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
fn agent_named(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    let file = lower.rsplit(['/', '\\']).next().unwrap_or(&lower);
    let file = file.strip_suffix(".exe").unwrap_or(file);
    AGENTS
        .iter()
        .find(|(exact, part, _)| file == *exact || lower.contains(part))
        .map(|(_, _, agent)| agent.to_string())
}

/// A command line for display: the program's file name, then its arguments,
/// cut to a readable length.
pub fn display_command<'a>(mut args: impl Iterator<Item = &'a str>) -> Option<String> {
    const MAX: usize = 200;
    let first = args.next()?;
    let program = first.rsplit(['/', '\\']).next().unwrap_or(first);
    let mut out = std::iter::once(program).chain(args).collect::<Vec<_>>().join(" ");
    if out.chars().count() > MAX {
        out = out.chars().take(MAX).collect::<String>() + "…";
    }
    Some(out)
}

/// One connection from a program asking for keys.
pub trait KeyStream: Read + Write + Send {}
impl<T: Read + Write + Send> KeyStream for T {}

/// Handles one program's connection; runs on its own thread.
pub type ServeKeys = std::sync::Arc<dyn Fn(Peer, Box<dyn KeyStream>) + Send + Sync>;
