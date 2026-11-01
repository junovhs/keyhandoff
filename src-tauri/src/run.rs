//! `keyhandoff run -- python report.py`: start any program with the keys its
//! `handoff:` pointers name, without changing its code. Pointers come from
//! the environment and `.env`, like the loaders. The running app is asked
//! over the key socket or pipe for the exact command, so the approval covers
//! that command and the files it names, nothing else.
//!
//! A key whose servers are known reaches the program as a placeholder. On
//! Linux the runner preloads KeyHandoff's TLS hook (`hook/keyhandoff_tls.c`,
//! built into this executable), which swaps the key in only inside a TLS
//! connection verified as one of those servers, and has Node.js load
//! `hook/keyhandoff_node.cjs`, since Node brings its own TLS. The runner keeps its
//! connection to KeyHandoff open while the program runs; closing it ends the
//! run, and the placeholders stop working.
//!
//! The program's stdout and stderr go through a [`Masker`] on their way to
//! the terminal, for it and everything it starts, so a key it prints shows
//! as `[KeyHandoff: Stripe hidden]`. Writes to files or the network aren't
//! covered.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;
use zeroize::Zeroizing;

use crate::mask::Masker;

const PREFIX: &str = "handoff:";
const USAGE: &str = "Usage: keyhandoff run [--profile NAME] [--env FILE] -- COMMAND [ARGS...]";

/// Where a run's output goes; tests capture it.
pub type Out = Box<dyn Write + Send>;

#[derive(Debug, PartialEq, Eq)]
struct Options {
    profile: String,
    env_file: PathBuf,
    command: Vec<String>,
}

#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    values: BTreeMap<String, String>,
    /// The token the TLS hook asks with, when a value is a placeholder.
    hook: Option<String>,
    error: Option<String>,
}

/// `keyhandoff run …` from a terminal; returns the exit code.
pub fn main(args: &[String]) -> i32 {
    match options(args) {
        Ok(options) => match run(&options, Box::new(std::io::stdout()), Box::new(std::io::stderr()), true) {
            Ok(code) => code,
            Err(error) => {
                eprintln!("keyhandoff: {error}");
                1
            }
        },
        Err(error) => {
            eprintln!("{error}");
            2
        }
    }
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        profile: std::env::var("KEYHANDOFF_PROFILE").unwrap_or_else(|_| "default".into()),
        env_file: ".env".into(),
        command: Vec::new(),
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => options.profile = args.next().ok_or(USAGE)?.clone(),
            "--env" => options.env_file = args.next().ok_or(USAGE)?.into(),
            "--" => break,
            "-h" | "--help" => return Err(USAGE.into()),
            _ => {
                options.command.push(arg.clone());
                break;
            }
        }
    }
    options.command.extend(args.cloned());
    if options.command.is_empty() {
        return Err(USAGE.into());
    }
    Ok(options)
}

/// Ask for the keys, start the command with them and mask its output.
/// `quiet_signals`: leave Ctrl+C to the program, as a terminal run should.
fn run(options: &Options, out: Out, err: Out, quiet_signals: bool) -> Result<i32, String> {
    let program = find_program(&options.command[0]).ok_or_else(|| format!("{} isn't a program KeyHandoff can find.", options.command[0]))?;
    let pointers = pointers(&options.env_file);
    let mut env: Vec<(String, Zeroizing<String>)> = Vec::new();
    let mut keys: Vec<(String, Zeroizing<String>)> = Vec::new();
    // Held open while the program runs: closing it ends the run.
    let mut _run = None;
    if !pointers.is_empty() {
        let mut names: Vec<&str> = pointers.values().map(String::as_str).collect();
        names.sort();
        names.dedup();
        let mut argv = vec![program.to_string_lossy().into_owned()];
        argv.extend(options.command[1..].iter().cloned());
        let (mut reply, conn) = ask(&options.profile, &names, &argv)?;
        for (var, name) in &pointers {
            let value = reply.values.get(name).ok_or_else(|| format!("KeyHandoff didn't send {name}."))?;
            env.push((var.clone(), Zeroizing::new(value.clone())));
        }
        for (name, value) in std::mem::take(&mut reply.values) {
            keys.push((name, Zeroizing::new(value)));
        }
        if let Some(token) = reply.hook {
            env.extend(hook_env(&options.profile, &token)?);
            _run = Some(conn);
        }
    }

    let mut child = Command::new(&program)
        .args(&options.command[1..])
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("couldn't start {}: {e}", options.command[0]))?;
    drop(env);
    if quiet_signals {
        ignore_interrupts();
    }
    let masker = || Masker::new(keys.iter().map(|(name, value)| (name.as_str(), value.as_bytes())));
    let stdout = pump(child.stdout.take().expect("piped"), out, masker());
    let stderr = pump(child.stderr.take().expect("piped"), err, masker());
    drop(keys);
    let status = child.wait().map_err(|e| e.to_string())?;
    let _ = stdout.join();
    let _ = stderr.join();
    Ok(exit_code(status))
}

/// Copy `from` to `to` through `masker`, on its own thread.
fn pump(mut from: impl Read + Send + 'static, mut to: Out, mut masker: Masker) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buf = Zeroizing::new(vec![0u8; 64 * 1024]);
        loop {
            match from.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let masked = masker.push(&buf[..n]);
                    if to.write_all(&masked).and_then(|()| to.flush()).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = to.write_all(&masker.finish());
        let _ = to.flush();
    })
}

/// Ask the running app for `names`, naming the command that will get them.
/// Returns the reply and the connection, which a run with placeholders holds.
fn ask(profile: &str, names: &[&str], argv: &[String]) -> Result<(Reply, Box<dyn crate::os::KeyStream>), String> {
    let mut conn = crate::os::connect_keys(profile).map_err(|_| "KeyHandoff isn't running. Open it and try again.".to_string())?;
    let mut message = serde_json::to_vec(&serde_json::json!({ "names": names, "run": argv, "placeholders": cfg!(target_os = "linux") })).map_err(|e| e.to_string())?;
    message.push(b'\n');
    conn.write_all(&message).map_err(|e| e.to_string())?;
    let mut line = Zeroizing::new(String::new());
    BufReader::new(&mut *conn).read_line(&mut line).map_err(|_| "KeyHandoff closed the connection.".to_string())?;
    let reply: Reply = serde_json::from_str(&line).map_err(|_| "KeyHandoff sent something unreadable.".to_string())?;
    match reply.error {
        Some(error) => Err(error),
        None => Ok((reply, conn)),
    }
}

/// The TLS hook, built from hook/keyhandoff_tls.c by build.rs.
#[cfg(target_os = "linux")]
const TLS_HOOK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libkeyhandoff_tls.so"));

/// The Node.js hook (hook/keyhandoff_node.cjs): Node brings its own TLS.
#[cfg(target_os = "linux")]
const NODE_HOOK: &[u8] = include_bytes!("../hook/keyhandoff_node.cjs");

/// The environment that loads the TLS hook into the program: written once
/// to a private folder in the runtime directory, named by its contents.
#[cfg(target_os = "linux")]
fn hook_env(profile: &str, token: &str) -> Result<Vec<(String, Zeroizing<String>)>, String> {
    use sha2::{Digest, Sha256};
    let dir = crate::os::runtime_dir().join("keyhandoff-hook");
    crate::os::private_dir(&dir).map_err(|e| format!("couldn't prepare the TLS hook: {e}"))?;
    let name: String = Sha256::digest(TLS_HOOK).iter().take(8).map(|b| format!("{b:02x}")).collect();
    let path = dir.join(format!("tls-{name}.so"));
    if std::fs::read(&path).ok().as_deref() != Some(TLS_HOOK) {
        crate::os::write_private(&path, TLS_HOOK).map_err(|e| format!("couldn't prepare the TLS hook: {e}"))?;
    }
    let node = dir.join(format!("node-{}.cjs", Sha256::digest(NODE_HOOK).iter().take(8).map(|b| format!("{b:02x}")).collect::<String>()));
    if std::fs::read(&node).ok().as_deref() != Some(NODE_HOOK) {
        crate::os::write_private(&node, NODE_HOOK).map_err(|e| format!("couldn't prepare the Node.js hook: {e}"))?;
    }
    let node_options = match std::env::var("NODE_OPTIONS") {
        Ok(old) if !old.is_empty() => format!("--require \"{}\" {old}", node.display()),
        _ => format!("--require \"{}\"", node.display()),
    };
    let socket = crate::os::key_socket(profile);
    let preload = match std::env::var("LD_PRELOAD") {
        Ok(old) if !old.is_empty() => format!("{}:{old}", path.display()),
        _ => path.display().to_string(),
    };
    Ok(vec![
        ("LD_PRELOAD".into(), Zeroizing::new(preload)),
        ("NODE_OPTIONS".into(), Zeroizing::new(node_options)),
        ("KEYHANDOFF_HOOK".into(), Zeroizing::new(format!("{token}:{}", socket.display()))),
    ])
}

/// Placeholders need the TLS hook, which is built for Linux only so far.
#[cfg(not(target_os = "linux"))]
fn hook_env(_profile: &str, _token: &str) -> Result<Vec<(String, Zeroizing<String>)>, String> {
    Err("Placeholders need KeyHandoff's TLS hook, which isn't built for this system yet.".into())
}

/// Variables whose value is a `handoff:` pointer, from the environment and
/// the .env file, mapped to the key names. A variable already set to a real
/// value is left alone, as the loaders do.
fn pointers(env_file: &Path) -> BTreeMap<String, String> {
    let mut pointers: BTreeMap<String, String> = std::env::vars()
        .filter_map(|(var, value)| value.strip_prefix(PREFIX).map(|name| (var, name.trim().to_string())))
        .collect();
    for (var, value) in read_env(env_file) {
        if let Some(name) = value.strip_prefix(PREFIX) {
            if std::env::var_os(&var).is_none() || pointers.contains_key(&var) {
                pointers.insert(var, name.trim().to_string());
            }
        }
    }
    pointers
}

/// `.env` lines, read the way the Python loader reads them.
fn read_env(path: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    let mut out = Vec::new();
    for line in text.lines() {
        let mut line = line.trim();
        if let Some(rest) = line.strip_prefix("export ") {
            line = rest.trim_start();
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((var, value)) = line.split_once('=') else { continue };
        let (var, mut value) = (var.trim(), value.trim());
        let bytes = value.as_bytes();
        if bytes.len() >= 2 && bytes[0] == bytes[bytes.len() - 1] && (bytes[0] == b'"' || bytes[0] == b'\'') {
            value = &value[1..value.len() - 1];
        } else if let Some((before, _)) = value.split_once(" #") {
            value = before.trim_end();
        }
        out.push((var.to_string(), value.to_string()));
    }
    out
}

/// The full path of `name`: as given when it has a folder in it, else the
/// first match on `PATH` (with `PATHEXT` on Windows). Links aren't followed,
/// so the prompt shows `python3` as you typed it.
fn find_program(name: &str) -> Option<PathBuf> {
    let given = Path::new(name);
    if given.components().count() > 1 {
        return std::path::absolute(given).ok().filter(|p| is_program(p));
    }
    let exts: Vec<String> = if cfg!(windows) {
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into());
        std::iter::once(String::new()).chain(pathext.split(';').map(str::to_lowercase)).collect()
    } else {
        vec![String::new()]
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .flat_map(|dir| exts.iter().map(move |ext| dir.join(format!("{name}{ext}"))))
        .find(|p| is_program(p))
        .and_then(|p| std::path::absolute(p).ok())
}

#[cfg(unix)]
fn is_program(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_program(p: &Path) -> bool {
    p.is_file()
}

#[cfg(target_os = "linux")]
fn ignore_interrupts() {
    // SAFETY: setting a signal to ignored has no handler to race with. The
    // program, already started, keeps its own handling.
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_IGN);
        libc::signal(libc::SIGQUIT, libc::SIG_IGN);
    }
}

#[cfg(not(target_os = "linux"))]
fn ignore_interrupts() {}

#[cfg(unix)]
fn exit_code(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status.code().or(status.signal().map(|s| 128 + s)).unwrap_or(1)
}

#[cfg(not(unix))]
fn exit_code(status: std::process::ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn options_take_the_command_after_the_flags() {
        let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let o = options(&args("--profile work -- python3 -c pass")).unwrap();
        assert_eq!((o.profile.as_str(), o.command.join(" ")), ("work", "python3 -c pass".into()));
        assert_eq!(options(&args("node app.js --profile x")).unwrap().command, args("node app.js --profile x"));
        assert!(options(&args("--profile")).is_err());
    }

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);
    impl Write for Capture {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Not a test on its own: the end-to-end tests start this test binary
    /// again with `KEYHANDOFF_TEST_RUN` set, so the runner is KeyHandoff's
    /// own executable and has its own parent chain, as `keyhandoff run` has.
    /// It runs the command and writes the output, then the exit code.
    #[test]
    fn runner_entry() {
        let Ok(spec) = std::env::var("KEYHANDOFF_TEST_RUN") else { return };
        let spec: Vec<String> = serde_json::from_str(&spec).unwrap();
        let (profile, out_path, command) = (&spec[0], PathBuf::from(&spec[1]), spec[2..].to_vec());
        let out = Capture::default();
        let options = Options { profile: profile.clone(), env_file: ".env".into(), command };
        let code = match run(&options, Box::new(out.clone()), Box::new(out.clone()), false) {
            Ok(code) => code,
            Err(error) => {
                out.clone().write_all(format!("keyhandoff: {error}\n").as_bytes()).unwrap();
                1
            }
        };
        std::fs::write(&out_path, &*out.0.lock().unwrap()).unwrap();
        std::fs::write(out_path.with_extension("code"), code.to_string()).unwrap();
    }

    /// Start `keyhandoff run` (this binary, as above) in `folder`, detached
    /// so it isn't a child of whatever started the tests, under `wrapper`
    /// (a shell to stand in for an agent) if given. Returns where its output
    /// and exit code will be.
    pub(crate) fn start(folder: &Path, profile: &str, command: &[&str], wrapper: Option<&Path>) -> PathBuf {
        let out = folder.join(format!("run-{}.out", rand_core::RngCore::next_u32(&mut rand_core::OsRng)));
        let mut spec = vec![profile.to_string(), out.display().to_string()];
        spec.extend(command.iter().map(|s| s.to_string()));
        let me = std::env::current_exe().unwrap();
        let runner = format!("'{}' run::tests::runner_entry --exact -q >/dev/null 2>&1", me.display());
        // `; true` keeps the wrapper shell alive as the runner's parent.
        let shell = wrapper.map(Path::to_path_buf).unwrap_or_else(|| "/bin/sh".into());
        std::process::Command::new("setsid")
            .args(["-f", &shell.display().to_string(), "-c", &format!("{runner}; true")])
            .current_dir(folder)
            .env("KEYHANDOFF_TEST_RUN", serde_json::to_string(&spec).unwrap())
            .env_remove("CLAUDECODE")
            .stdin(Stdio::null())
            .status()
            .unwrap();
        out
    }

    /// The output and exit code of a run [`start`]ed, once it's done.
    pub(crate) fn finished(out: &Path) -> (String, i32) {
        let code = out.with_extension("code");
        for _ in 0..3000 {
            if let Ok(code) = std::fs::read_to_string(&code) {
                return (std::fs::read_to_string(out).unwrap(), code.parse().unwrap());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("keyhandoff run didn't finish");
    }
}
