//! A copy of the master key held by the OS: the Secret Service (GNOME
//! Keyring or KWallet), or a 0600 file without one. Both hand it to any
//! program running as you. Profiles from before key slots kept it here (read
//! once, moved behind the passphrase, then deleted); "Remember on this
//! computer" keeps one here on purpose.

use std::fs;
use std::path::Path;

use rand_core::{OsRng, RngCore};
use zeroize::Zeroizing;

use crate::os::MasterKey;

pub const SECRET_SERVICE: &str = "secret-service";
pub const FILE: &str = "file";
const SERVICE: &str = "Handoff";

/// Load or create the master key. `backend` is what an earlier run used.
pub fn master_key(dir: &Path, profile: &str, backend: Option<&str>) -> Result<(MasterKey, &'static str), String> {
    match backend {
        Some(FILE) => file_key(dir).map(|k| (k, FILE)),
        Some(_) => secret_service_key(profile).map(|k| (k, SECRET_SERVICE)),
        None => match secret_service_key(profile) {
            Ok(k) => Ok((k, SECRET_SERVICE)),
            Err(e) => {
                eprintln!("keyhandoff: no Secret Service ({e}); keeping the master key in a private file");
                file_key(dir).map(|k| (k, FILE))
            }
        },
    }
}

/// Keep a copy of `key` for "Remember on this computer". `prefer` picks the
/// backend; otherwise the Secret Service, or the file without one.
pub fn remember_master_key(dir: &Path, profile: &str, key: &MasterKey, prefer: Option<&str>) -> Result<&'static str, String> {
    if prefer != Some(FILE) {
        let saved = keyring::Entry::new(SERVICE, profile).and_then(|e| e.set_secret(&key.0[..]));
        match saved {
            Ok(()) => return Ok(SECRET_SERVICE),
            Err(e) => eprintln!("keyhandoff: no Secret Service ({e}); remembering the master key in a private file"),
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
    match keyring::Entry::new(SERVICE, profile).and_then(|e| e.get_secret()) {
        Ok(bytes) => from_vec(Zeroizing::new(bytes)).map(Some),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("The system keyring could not be opened: {e}")),
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
    let entry = keyring::Entry::new(SERVICE, profile).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn secret_service_key(profile: &str) -> Result<MasterKey, String> {
    let entry = keyring::Entry::new(SERVICE, profile).map_err(|e| e.to_string())?;
    match entry.get_secret() {
        Ok(bytes) => from_vec(Zeroizing::new(bytes)),
        Err(keyring::Error::NoEntry) => {
            let key = fresh();
            entry.set_secret(&key.0[..]).map_err(|e| e.to_string())?;
            Ok(key)
        }
        Err(e) => Err(format!("The system keyring could not be opened: {e}")),
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
