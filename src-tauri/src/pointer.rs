//! `handoff:Name` pointers. A program sends one line of JSON,
//! `{"names": ["Dialpad"]}`, over the key socket or pipe, and gets one line
//! back: `{"values": {"Dialpad": "…"}}` or `{"error": "…"}`. The loader in
//! `loaders/` reads the pointers in `.env` and sets the variables in its own
//! process, so the values never touch the disk.
//!
//! `keyhandoff run` asks the same way and adds the exact command it will
//! start, `{"names": […], "run": ["/usr/bin/python3", "report.py"]}`. That's
//! believed only from KeyHandoff's own executable: the approval is then for
//! that command, which is what really runs. A key whose servers are known
//! comes back as a placeholder, with `"hook": "<token>"`; the runner keeps
//! the connection open while the program runs, and the run ends when it
//! closes.
//!
//! The TLS hook in that program (`hook/keyhandoff_tls.c`) asks with the
//! token: `{"hook": "<token>"}` gets one `placeholder<TAB>host,host<TAB>ca`
//! line per placeholder, then an empty line, and
//! `{"swap": {"token", "placeholder", "host"}}` gets `ok <key in hex>` or
//! `no <reason>`, only from a process the runner started.

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
    #[serde(default)]
    names: Vec<String>,
    /// From `keyhandoff run`: the program, resolved to a full path, and its arguments.
    #[serde(default)]
    run: Option<Vec<String>>,
    /// From `keyhandoff run` when it can preload the TLS hook: placeholders are welcome.
    #[serde(default)]
    placeholders: bool,
    /// From the TLS hook: its run's placeholders.
    #[serde(default)]
    hook: Option<String>,
    /// From the TLS hook: the key behind a placeholder, for a verified host.
    #[serde(default)]
    swap: Option<SwapAsk>,
}

#[derive(Deserialize)]
struct SwapAsk {
    token: String,
    placeholder: String,
    host: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Reply<'a> {
    Values {
        values: BTreeMap<&'a str, &'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        hook: Option<&'a str>,
    },
    Error {
        error: String,
    },
}

/// Answer one program's connection.
pub fn serve(handoff: &Handoff, peer: &Peer, mut stream: Box<dyn KeyStream>) {
    let mut line = String::new();
    let read = BufReader::new((&mut *stream).take(MAX_ASK_BYTES)).read_line(&mut line);
    let ask = read.ok().and_then(|_| serde_json::from_str::<Ask>(&line).ok());
    let mut run = None;
    let reply = match ask {
        Some(Ask { hook: Some(token), .. }) => hook_lines(handoff, peer, &token),
        Some(Ask { swap: Some(swap), .. }) => swap_line(handoff, peer, &swap),
        Some(ask) if !ask.names.is_empty() && ask.names.len() <= MAX_NAMES => answer(handoff, peer, ask.names, ask.run, ask.placeholders, &mut run),
        _ => reply(&Reply::Error { error: "KeyHandoff couldn't read the request.".into() }),
    };
    let _ = stream.write_all(&reply);
    let _ = stream.flush();
    drop(reply);
    if let Some(token) = run {
        // The run lasts as long as the runner's connection.
        let mut sink = [0u8; 64];
        while matches!(stream.read(&mut sink), Ok(n) if n > 0) {}
        handoff.end_run(&token);
    }
}

/// Turn a program away with `error`.
pub fn refuse(mut stream: Box<dyn KeyStream>, error: &str) {
    let _ = stream.write_all(&reply(&Reply::Error { error: error.into() }));
    let _ = stream.flush();
}

fn hook_lines(handoff: &Handoff, peer: &Peer, token: &str) -> Zeroizing<Vec<u8>> {
    let mut out = Zeroizing::new(Vec::new());
    for (placeholder, hosts, ca) in handoff.hook_swaps(token, peer).unwrap_or_default() {
        let ca = ca.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        if ca.contains(['\t', '\n']) {
            continue;
        }
        out.extend_from_slice(format!("{}\t{}\t{}\n", &*placeholder, hosts.join(","), ca).as_bytes());
    }
    out.push(b'\n');
    out
}

fn swap_line(handoff: &Handoff, peer: &Peer, swap: &SwapAsk) -> Zeroizing<Vec<u8>> {
    match handoff.swap(&swap.token, peer, &swap.placeholder, &swap.host) {
        Ok(secret) => {
            let mut out = Zeroizing::new(Vec::with_capacity(4 + 2 * secret.expose().len()));
            out.extend_from_slice(b"ok ");
            for b in secret.expose() {
                out.extend_from_slice(&[HEX[(b >> 4) as usize], HEX[(b & 15) as usize]]);
            }
            out.push(b'\n');
            out
        }
        Err(error) => Zeroizing::new(format!("no {}\n", error.replace('\n', " ")).into_bytes()),
    }
}

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Values for a loader, or for `keyhandoff run`, which may get placeholders
/// and a hook token (put in `started`).
fn answer(
    handoff: &Handoff,
    peer: &Peer,
    names: Vec<String>,
    run: Option<Vec<String>>,
    placeholders: bool,
    started: &mut Option<String>,
) -> Zeroizing<Vec<u8>> {
    let ran;
    let is_run = run.is_some() && placeholders;
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
    let (secrets, token) = if is_run { handoff.begin_run(peer, &names, secrets) } else { (secrets, None) };
    let mut values = BTreeMap::new();
    for (name, secret) in names.iter().zip(&secrets) {
        let Ok(value) = std::str::from_utf8(secret.expose()) else {
            if let Some(token) = &token {
                handoff.end_run(token);
            }
            return reply(&Reply::Error { error: format!("{name} isn't text, so it can't go in an environment variable.") });
        };
        values.insert(name.as_str(), value);
    }
    let out = reply(&Reply::Values { values, hook: token.as_deref() });
    *started = token;
    out
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
        pid: runner.pid,
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
        Reply::Values { values, .. } => values.iter().map(|(k, v)| 6 * (k.len() + v.len()) + 8).sum(),
        Reply::Error { .. } => 0,
    };
    let mut out = Zeroizing::new(Vec::with_capacity(values + 1024));
    serde_json::to_writer(&mut *out, reply).expect("a reply serializes");
    out.push(b'\n');
    out
}
