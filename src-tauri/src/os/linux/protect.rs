//! The profile's master key lives in the Secret Service (GNOME Keyring or
//! KWallet), unlocked with the user's login. Without a Secret Service the key
//! falls back to a 0600 file, which only protects against other users; the
//! backend in use is recorded so a later run never silently switches.

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
