//! `handoff:Name` pointers. A program sends one line of JSON,
//! `{"names": ["Dialpad"]}`, over the key socket or pipe, and gets one line
//! back: `{"values": {"Dialpad": "…"}}` or `{"error": "…"}`. The loader in
//! `loaders/` reads the pointers in `.env` and sets the variables in its own
//! process, so the values never touch the disk.
//!
//! `keyhandoff run` asks the same way and adds the exact command it will
//! start, `{"names": […], "run": ["/usr/bin/python3", "report.py"]}`. That's
//! believed only from KeyHandoff's own executable: the approval is then for
//! that command, which is what really runs.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::core::Handoff;
use crate::os::{KeyStream, Peer};

const MAX_ASK_BYTES: u64 = 16 * 1024;
const MAX_NAMES: usize = 64;
const PREFIX: &str = "handoff:";

#[derive(Deserialize)]
struct Ask {
    names: Vec<String>,
    /// From `keyhandoff run`: the program, resolved to a full path, and its arguments.
    #[serde(default)]
    run: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Reply<'a> {
    Values { values: BTreeMap<&'a str, &'a str> },
    Error { error: String },
}

/// Answer one program's connection.
pub fn serve(handoff: &Handoff, peer: &Peer, mut stream: Box<dyn KeyStream>) {
    let reply = answer(handoff, peer, &mut *stream);
    let _ = stream.write_all(&reply);
    let _ = stream.flush();
}

/// Turn a program away with `error`.
pub fn refuse(mut stream: Box<dyn KeyStream>, error: &str) {
    let _ = stream.write_all(&reply(&Reply::Error { error: error.into() }));
    let _ = stream.flush();
}

fn answer(handoff: &Handoff, peer: &Peer, stream: &mut dyn KeyStream) -> Zeroizing<Vec<u8>> {
    let mut line = String::new();
    let read = BufReader::new(stream.take(MAX_ASK_BYTES)).read_line(&mut line);
    let (names, run) = match read.ok().and_then(|_| serde_json::from_str::<Ask>(&line).ok()) {
        Some(ask) if !ask.names.is_empty() && ask.names.len() <= MAX_NAMES => (ask.names, ask.run),
        _ => return reply(&Reply::Error { error: "KeyHandoff couldn't read the request.".into() }),
    };
    let ran;
    let peer = match run {
        None => peer,
        Some(argv) => match running(peer, argv) {
            Ok(p) => {
                ran = p;
                &ran
            }
            Err(error) => return reply(&Reply::Error { error }),
        },
    };
    // Accept a bare name or the whole pointer.
    let names: Vec<String> = names.iter().map(|n| n.strip_prefix(PREFIX).unwrap_or(n).trim().to_string()).collect();
    let secrets = match handoff.give(peer, &names) {
        Ok(secrets) => secrets,
        Err(error) => return reply(&Reply::Error { error }),
    };
    let mut values = BTreeMap::new();
    for (name, secret) in names.iter().zip(&secrets) {
        let Ok(value) = std::str::from_utf8(secret.expose()) else {
            return reply(&Reply::Error { error: format!("{name} isn't text, so it can't go in an environment variable.") });
        };
        values.insert(name.as_str(), value);
    }
    reply(&Reply::Values { values })
}

/// The program `keyhandoff run` is about to start, in its folder, if the
/// request really comes from KeyHandoff's own executable: the same file, or
/// one with the same contents (an AppImage runs from a new path each time).
fn running(runner: &Peer, argv: Vec<String>) -> Result<Peer, String> {
    if !is_own_executable(&runner.program) {
        return Err("Only keyhandoff run can name the command it starts.".into());
    }
    let Some((program, args)) = argv.split_first() else {
        return Err("keyhandoff run didn't name a command.".into());
    };
    let program = std::path::PathBuf::from(program);
    if !program.is_absolute() {
        return Err("keyhandoff run didn't name a command.".into());
    }
    Ok(Peer {
        command: crate::os::display_command(argv.iter().map(String::as_str)),
        program,
        args: args.to_vec(),
        folder: runner.folder.clone(),
        agent: runner.agent.clone(),
    })
}

fn is_own_executable(program: &std::path::Path) -> bool {
    static OWN: std::sync::OnceLock<Option<[u8; 32]>> = std::sync::OnceLock::new();
    let Ok(exe) = std::env::current_exe() else { return false };
    if same_file(&exe, program) {
        return true;
    }
    let own = OWN.get_or_init(|| crate::os::file_hash(&exe));
    own.is_some() && crate::os::file_hash(program) == *own
}

#[cfg(unix)]
fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    matches!((a.metadata(), b.metadata()), (Ok(a), Ok(b)) if a.dev() == b.dev() && a.ino() == b.ino())
}

#[cfg(not(unix))]
fn same_file(a: &std::path::Path, b: &std::path::Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

/// One line of JSON, in a buffer big enough that it never reallocates (which
/// would leave copies of a value behind), zeroized when dropped.
fn reply(reply: &Reply) -> Zeroizing<Vec<u8>> {
    let values = match reply {
        Reply::Values { values } => values.iter().map(|(k, v)| 6 * (k.len() + v.len()) + 8).sum(),
        Reply::Error { .. } => 0,
    };
    let mut out = Zeroizing::new(Vec::with_capacity(values + 1024));
    serde_json::to_writer(&mut *out, reply).expect("a reply serializes");
    out.push(b'\n');
    out
}
