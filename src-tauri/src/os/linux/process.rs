//! Starting a program under `keyhandoff run` on Linux: the TLS and Node.js
//! hooks it loads, and the process details `run.rs` needs.

use std::path::Path;

use zeroize::Zeroizing;

/// The TLS hook, built from hook/keyhandoff_tls.c by build.rs.
const TLS_HOOK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libkeyhandoff_tls.so"));

/// The Node.js hook (hook/keyhandoff_node.cjs): Node brings its own TLS.
const NODE_HOOK: &[u8] = include_bytes!("../../../hook/keyhandoff_node.cjs");

/// The environment that loads the TLS hook into the program: written once
/// to a private folder in the runtime directory, named by its contents.
pub fn hook_env(profile: &str, token: &str) -> Result<Vec<(String, Zeroizing<String>)>, String> {
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

pub fn hooked(_program: &Path) -> bool {
    true
}

pub fn is_program(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

pub fn ignore_interrupts() {
    // SAFETY: setting a signal to ignored has no handler to race with. The
    // program, already started, keeps its own handling.
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_IGN);
        libc::signal(libc::SIGQUIT, libc::SIG_IGN);
    }
}

pub fn exit_code(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status.code().or(status.signal().map(|s| 128 + s)).unwrap_or(1)
}

/// The endings a program's file name may have, tried in order on `PATH`.
pub fn program_suffixes() -> Vec<String> {
    vec![String::new()]
}

/// Whether two paths are the same file (same device and inode).
pub fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    matches!((a.metadata(), b.metadata()), (Ok(a), Ok(b)) if a.dev() == b.dev() && a.ino() == b.ino())
}

/// Open a web address in the default browser.
pub fn open_url(url: &str) -> std::io::Result<()> {
    std::process::Command::new("xdg-open").arg(url).spawn().map(|_| ())
}
