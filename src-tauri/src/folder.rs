//! A team's shared folder: the files KeyHandoff reads and writes there.
//! Whatever syncs the folder (Dropbox, a repo, a network share) moves them;
//! they hold only public keys and ciphertext, as armored text.
//!
//! - `invite.handoff`: the keeper's invite.
//! - `members/<fingerprint>.handoff`: each joiner's membership.
//! - `team.handoff`: the latest team state.
//! - `warnings/<fingerprint>-<destination hash>.handoff`: a member's
//!   warning about a destination one of their programs tried to send a
//!   placeholder to.

use std::fs;
use std::path::Path;

use keyhandoff_seal::{armor, Invite, Membership, Object, TeamState, Warning};
use rand_core::{OsRng, RngCore};

const INVITE: &str = "invite.handoff";
const STATE: &str = "team.handoff";
const MEMBERS: &str = "members";
const WARNINGS: &str = "warnings";
/// Larger files in the folder are not KeyHandoff's and are skipped.
const MAX_FILE_BYTES: u64 = 1024 * 1024;

pub fn write_invite(folder: &Path, invite: &Invite) -> Result<(), String> {
    let line = format!("{} invited you to {}. Open this with KeyHandoff. It holds no secret.", invite.keeper_name(), invite.team());
    write(&folder.join(INVITE), invite.to_bytes(), &line)
}

pub fn write_membership(folder: &Path, membership: &Membership) -> Result<(), String> {
    let line = format!("{} asks to join the team. It holds no secret.", membership.name());
    write(&folder.join(MEMBERS).join(format!("{}.handoff", membership.member().fingerprint())), membership.to_bytes(), &line)
}

pub fn write_state(folder: &Path, state: &TeamState) -> Result<(), String> {
    let line = format!("The {} team, version {}. Only its members' KeyHandoff can open it.", state.team(), state.version());
    write(&folder.join(STATE), state.to_bytes(), &line)
}

pub fn write_warning(folder: &Path, warning: &Warning) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let tag: String = Sha256::digest(warning.destination().as_bytes()).iter().take(6).map(|b| format!("{b:02x}")).collect();
    let line = format!("A program tried to send a key's stand-in to {}. KeyHandoff refuses it for the team. It holds no secret.", warning.destination());
    write(&folder.join(WARNINGS).join(format!("{}-{tag}.handoff", warning.reporter().fingerprint())), warning.to_bytes(), &line)
}

/// Every warning in the folder that parses and verifies.
pub fn warnings(folder: &Path) -> Vec<Warning> {
    let Ok(entries) = fs::read_dir(folder.join(WARNINGS)) else { return Vec::new() };
    entries
        .flatten()
        .take(1024)
        .filter_map(|e| match read(&e.path()) {
            Some(Object::Warning(w)) => Some(w),
            _ => None,
        })
        .collect()
}

/// Every membership in the folder that parses and verifies. Anything else is skipped.
pub fn memberships(folder: &Path) -> Vec<Membership> {
    let Ok(entries) = fs::read_dir(folder.join(MEMBERS)) else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|e| match read(&e.path()) {
            Some(Object::Membership(m)) => Some(m),
            _ => None,
        })
        .collect()
}

pub fn state(folder: &Path) -> Option<TeamState> {
    match read(&folder.join(STATE)) {
        Some(Object::TeamState(s)) => Some(s),
        _ => None,
    }
}

fn read(path: &Path) -> Option<Object> {
    if fs::metadata(path).ok()?.len() > MAX_FILE_BYTES {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    keyhandoff_seal::parse_object(&armor::decode(&text).ok()?).ok()
}

/// Write through a temporary file and a rename, so a sync tool never picks up
/// half a file.
fn write(path: &Path, object: &[u8], line: &str) -> Result<(), String> {
    let text = armor::encode(object, line).map_err(|_| "That team is too big for one file.".to_string())?;
    let dir = path.parent().ok_or("That folder can't be used.")?;
    fs::create_dir_all(dir).map_err(|_| "KeyHandoff can't write to that folder.".to_string())?;
    let mut n = [0u8; 8];
    OsRng.fill_bytes(&mut n);
    let temp = dir.join(format!(".keyhandoff-{}.tmp", u64::from_le_bytes(n)));
    fs::write(&temp, text).map_err(|_| "KeyHandoff can't write to that folder.".to_string())?;
    fs::rename(&temp, path).map_err(|_| {
        let _ = fs::remove_file(&temp);
        "KeyHandoff can't write to that folder.".to_string()
    })
}
