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

/// Put the `keyhandoff` command on `PATH`, as the Windows installer and the
/// Linux packages do: a link to the app's own binary in `/usr/local/bin` (or
/// Homebrew's `bin` on Apple silicon), when that folder exists and you can
/// write to it without a password. Only runs from an app bundle, and only
/// ever replaces a link into a KeyHandoff app (an older copy, moved), never
/// a file of yours. Returns where it linked, if anywhere.
pub fn install_command() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    if !exe.to_string_lossy().contains(".app/Contents/MacOS/") {
        return None;
    }
    ["/usr/local/bin", "/opt/homebrew/bin"].into_iter().map(Path::new).find_map(|dir| link_into(dir, &exe))
}

fn link_into(dir: &Path, exe: &Path) -> Option<std::path::PathBuf> {
    let link = dir.join("keyhandoff");
    match std::fs::read_link(&link) {
        Ok(target) if target == exe => return Some(link),
        Ok(target) if target.to_string_lossy().contains(".app/Contents/MacOS/") => {
            std::fs::remove_file(&link).ok()?;
        }
        Ok(_) => return None,
        Err(_) if link.symlink_metadata().is_ok() => return None,
        Err(_) => {}
    }
    if !dir.is_dir() {
        return None;
    }
    std::os::unix::fs::symlink(exe, &link).ok()?;
    Some(link)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_link_replaces_only_its_own() {
        let dir = std::env::temp_dir().join(format!("kh-link-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = Path::new("/Applications/KeyHandoff.app/Contents/MacOS/keyhandoff");
        let link = dir.join("keyhandoff");
        // Fresh: linked.
        assert_eq!(link_into(&dir, exe), Some(link.clone()));
        assert_eq!(std::fs::read_link(&link).unwrap(), exe);
        // An older copy elsewhere: moved to this one.
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink("/Users/a/Downloads/KeyHandoff.app/Contents/MacOS/keyhandoff", &link).unwrap();
        assert_eq!(link_into(&dir, exe), Some(link.clone()));
        assert_eq!(std::fs::read_link(&link).unwrap(), exe);
        // Someone else's command of that name: left alone.
        std::fs::remove_file(&link).unwrap();
        std::fs::write(&link, "#!/bin/sh\n").unwrap();
        assert_eq!(link_into(&dir, exe), None);
        assert_eq!(std::fs::read_to_string(&link).unwrap(), "#!/bin/sh\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
