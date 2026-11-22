//! A copy of the master key held by the OS: a generic password in the login
//! Keychain, or a 0600 file when the Keychain can't be reached (a headless
//! session with it locked). The Keychain item is readable by KeyHandoff
//! itself without asking; another program reading it makes macOS ask you
//! first, unlike Linux's Secret Service or Windows' DPAPI. Profiles from
//! before key slots kept it here (read once, moved behind the passphrase,
//! then deleted); "Remember on this computer" keeps one here on purpose.

use std::fs;
use std::path::Path;

use rand_core::{OsRng, RngCore};
use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};
use zeroize::Zeroizing;

use crate::os::MasterKey;

pub const KEYCHAIN: &str = "keychain";
pub const FILE: &str = "file";
const SERVICE: &str = "KeyHandoff";
/// errSecItemNotFound.
const NOT_FOUND: i32 = -25300;

/// Load or create the master key. `backend` is what an earlier run used.
pub fn master_key(dir: &Path, profile: &str, backend: Option<&str>) -> Result<(MasterKey, &'static str), String> {
    match backend {
        Some(FILE) => file_key(dir).map(|k| (k, FILE)),
        Some(_) => keychain_key(profile).map(|k| (k, KEYCHAIN)),
        None => match keychain_key(profile) {
            Ok(k) => Ok((k, KEYCHAIN)),
            Err(e) => {
                eprintln!("keyhandoff: no Keychain ({e}); keeping the master key in a private file");
                file_key(dir).map(|k| (k, FILE))
            }
        },
    }
}

/// Keep a copy of `key` for "Remember on this computer". `prefer` picks the
/// backend; otherwise the Keychain, or the file without one.
pub fn remember_master_key(dir: &Path, profile: &str, key: &MasterKey, prefer: Option<&str>) -> Result<&'static str, String> {
    if prefer != Some(FILE) {
        match set_generic_password(SERVICE, profile, &key.0[..]) {
            Ok(()) => return Ok(KEYCHAIN),
            Err(e) => eprintln!("keyhandoff: no Keychain ({e}); remembering the master key in a private file"),
        }
    }
    super::write_private(&dir.join("master.key"), &key.0[..]).map_err(|e| e.to_string())?;
    Ok(FILE)
}

/// The remembered copy, if it's still there.
pub fn remembered_master_key(dir: &Path, profile: &str, backend: &str) -> Result<Option<MasterKey>, String> {
    if backend == FILE {
        return match fs::read(dir.join("master.key")) {
            Ok(bytes) => from_vec(Zeroizing::new(bytes)).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        };
    }
    match get_generic_password(SERVICE, profile) {
        Ok(bytes) => from_vec(Zeroizing::new(bytes)).map(Some),
        Err(e) if e.code() == NOT_FOUND => Ok(None),
        Err(e) => Err(format!("The Keychain could not be opened: {e}")),
    }
}

/// Delete the OS copy: after moving it into slots, or when you stop remembering.
pub fn forget_master_key(dir: &Path, profile: &str, backend: &str) -> Result<(), String> {
    if backend == FILE {
        return match fs::remove_file(dir.join("master.key")) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    match delete_generic_password(SERVICE, profile) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == NOT_FOUND => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn keychain_key(profile: &str) -> Result<MasterKey, String> {
    match get_generic_password(SERVICE, profile) {
        Ok(bytes) => from_vec(Zeroizing::new(bytes)),
        Err(e) if e.code() == NOT_FOUND => {
            let key = fresh();
            set_generic_password(SERVICE, profile, &key.0[..]).map_err(|e| e.to_string())?;
            Ok(key)
        }
        Err(e) => Err(format!("The Keychain could not be opened: {e}")),
    }
}

fn file_key(dir: &Path) -> Result<MasterKey, String> {
    let path = dir.join("master.key");
    match fs::read(&path) {
        Ok(bytes) => from_vec(Zeroizing::new(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let key = fresh();
            super::write_private(&path, &key.0[..]).map_err(|e| e.to_string())?;
            Ok(key)
        }
        Err(e) => Err(e.to_string()),
    }
}

fn from_vec(bytes: Zeroizing<Vec<u8>>) -> Result<MasterKey, String> {
    let arr: [u8; 32] = bytes[..].try_into().map_err(|_| "The stored master key is damaged.".to_string())?;
    Ok(MasterKey(Zeroizing::new(arr)))
}

fn fresh() -> MasterKey {
    let mut key = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(&mut key[..]);
    MasterKey(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real login Keychain: remember, read back, forget.
    #[test]
    #[ignore = "uses the login Keychain"]
    fn keychain_remembers_and_forgets() {
        let profile = format!("test-keychain-{}", std::process::id());
        let dir = std::env::temp_dir();
        let key = MasterKey::for_tests();
        assert_eq!(remember_master_key(&dir, &profile, &key, None).unwrap(), KEYCHAIN);
        let back = remembered_master_key(&dir, &profile, KEYCHAIN).unwrap().unwrap();
        assert_eq!(&back.0[..], &key.0[..]);
        forget_master_key(&dir, &profile, KEYCHAIN).unwrap();
        assert!(remembered_master_key(&dir, &profile, KEYCHAIN).unwrap().is_none());
        forget_master_key(&dir, &profile, KEYCHAIN).unwrap();
    }
}
