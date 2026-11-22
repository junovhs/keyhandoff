//! Starting a program under `keyhandoff run` on macOS: the Node.js hook it
//! loads, and the process details `run.rs` needs.
//!
//! Only Node.js programs get stand-ins here, as on Windows: the Linux TLS
//! hook is preloaded with `LD_PRELOAD`, and macOS's `DYLD_INSERT_LIBRARIES`
//! is stripped for every program System Integrity Protection covers
//! (`/usr/bin/python3`, `/usr/bin/curl`, `/usr/bin/env` in a shebang), so it
//! can't be a promise. Other programs get the real key, with printed copies
//! masked. The planned service proxy (FEAT-18) covers them all alike.

use std::path::Path;

use zeroize::Zeroizing;

/// The Node.js hook (hook/keyhandoff_node.cjs): Node brings its own TLS.
const NODE_HOOK: &[u8] = include_bytes!("../../../hook/keyhandoff_node.cjs");

/// Node.js and the tools that start it. Their launchers are scripts run by
/// `node`, so `NODE_OPTIONS` reaches it (SIP strips only `DYLD_` variables).
pub fn hooked(program: &Path) -> bool {
    let name = program.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    ["node", "npm", "npx", "pnpm", "yarn", "tsx"].contains(&name.as_str())
}

/// The environment that loads the Node.js hook: written once to a private
/// folder in the per-user temporary folder, named by its contents.
pub fn hook_env(profile: &str, token: &str) -> Result<Vec<(String, Zeroizing<String>)>, String> {
    use sha2::{Digest, Sha256};
    let dir = crate::os::runtime_dir().join("keyhandoff-hook");
    crate::os::private_dir(&dir).map_err(|e| format!("couldn't prepare the Node.js hook: {e}"))?;
    let node = dir.join(format!("node-{}.cjs", Sha256::digest(NODE_HOOK).iter().take(8).map(|b| format!("{b:02x}")).collect::<String>()));
    if std::fs::read(&node).ok().as_deref() != Some(NODE_HOOK) {
        crate::os::write_private(&node, NODE_HOOK).map_err(|e| format!("couldn't prepare the Node.js hook: {e}"))?;
    }
    let node_options = match std::env::var("NODE_OPTIONS") {
        Ok(old) if !old.is_empty() => format!("--require \"{}\" {old}", node.display()),
        _ => format!("--require \"{}\"", node.display()),
    };
    Ok(vec![
        ("NODE_OPTIONS".into(), Zeroizing::new(node_options)),
        ("KEYHANDOFF_HOOK".into(), Zeroizing::new(format!("{token}:{}", crate::os::key_socket(profile).display()))),
    ])
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
    std::process::Command::new("/usr/bin/open").arg(url).spawn().map(|_| ())
}
