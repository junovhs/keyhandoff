//! Starting a program under `keyhandoff run` on Windows: the Node.js hook it
//! loads, and the process details `run.rs` needs.

use std::path::Path;

use zeroize::Zeroizing;

/// On Windows only Node.js is hooked so far (`hook/keyhandoff_node.cjs`,
/// which needs no native code): placeholders go to a run whose program is
/// Node or a Node launcher, and every other program gets the key itself.
const NODE_HOOK: &[u8] = include_bytes!("../../../hook/keyhandoff_node.cjs");

/// Whether `program` is Node.js or one of the launchers that start it.
pub fn hooked(program: &Path) -> bool {
    let name = program.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    ["node.exe", "npm.cmd", "npx.cmd", "pnpm.cmd", "pnpm.exe", "yarn.cmd", "tsx.cmd"].contains(&name.as_str())
}

/// The environment that loads the Node.js hook: written once to the temp
/// folder, named by its contents, with the key pipe to ask over.
pub fn hook_env(profile: &str, token: &str) -> Result<Vec<(String, Zeroizing<String>)>, String> {
    use sha2::{Digest, Sha256};
    #[cfg(test)]
    if let Ok(port) = std::env::var("KEYHANDOFF_TEST_PROXY_PORT") {
        let port: u16 = port.parse().map_err(|_| "Invalid loopback fixture port")?;
        return Ok(vec![
            ("HTTPS_PROXY".into(), Zeroizing::new(format!("http://127.0.0.1:{port}"))),
            ("NO_PROXY".into(), Zeroizing::new(String::new())),
        ]);
    }
    let dir = crate::os::runtime_dir().join("keyhandoff-hook");
    crate::os::private_dir(&dir).map_err(|e| format!("couldn't prepare the Node.js hook: {e}"))?;
    let node = dir.join(format!("node-{}.cjs", Sha256::digest(NODE_HOOK).iter().take(8).map(|b| format!("{b:02x}")).collect::<String>()));
    if std::fs::read(&node).ok().as_deref() != Some(NODE_HOOK) {
        crate::os::write_private(&node, NODE_HOOK).map_err(|e| format!("couldn't prepare the Node.js hook: {e}"))?;
    }
    // NODE_OPTIONS reads backslashes as escapes; Node takes forward slashes.
    let require = node.display().to_string().replace('\\', "/");
    let node_options = match std::env::var("NODE_OPTIONS") {
        Ok(old) if !old.is_empty() => format!("--require \"{require}\" {old}"),
        _ => format!("--require \"{require}\""),
    };
    Ok(vec![
        ("NODE_OPTIONS".into(), Zeroizing::new(node_options)),
        ("KEYHANDOFF_HOOK".into(), Zeroizing::new(format!("{token}:{}", crate::os::key_socket(profile)))),
    ])
}

pub fn is_program(p: &Path) -> bool {
    p.is_file()
}

/// Ctrl+C reaches every program on the console; the program decides, and
/// the runner stays to pass on its output and exit code.
pub fn ignore_interrupts() {
    // SAFETY: no handler, TRUE: this process ignores Ctrl+C. Programs it
    // starts get their own default handling back.
    let _ = unsafe { ::windows::Win32::System::Console::SetConsoleCtrlHandler(None, true) };
}

pub fn exit_code(status: std::process::ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

/// The endings a program's file name may have, tried in order on `PATH`:
/// none, then `PATHEXT`'s.
pub fn program_suffixes() -> Vec<String> {
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into());
    std::iter::once(String::new()).chain(pathext.split(';').map(str::to_lowercase)).collect()
}

/// Whether two paths are the same file.
pub fn same_file(a: &Path, b: &Path) -> bool {
    matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

/// Open a web address in the default browser.
pub fn open_url(url: &str) -> std::io::Result<()> {
    std::process::Command::new("explorer").arg(url).spawn().map(|_| ())
}
