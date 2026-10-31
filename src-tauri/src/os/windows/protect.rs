//! Where profiles from before key slots kept their master key: wrapped with
//! DPAPI (current user) in `master.key`, with entropy naming the profile. Any
//! process running as this Windows user can unwrap it, so it's read once to
//! move it behind the passphrase (`slots`), then deleted here.

use std::fs;
use std::path::Path;

use ::windows::Win32::Foundation::{LocalFree, HLOCAL};
use ::windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
use rand_core::{OsRng, RngCore};
use zeroize::{Zeroize, Zeroizing};

use crate::os::MasterKey;

pub const DPAPI: &str = "dpapi";
const FILE: &str = "master.key";
const ENTROPY_PREFIX: &[u8] = b"Handoff master key v0/";

/// Load or create the master key. Windows has one backend, so `backend` is
/// only checked: a profile made elsewhere is refused rather than replaced.
pub fn master_key(dir: &Path, profile: &str, backend: Option<&str>) -> Result<(MasterKey, &'static str), String> {
    if let Some(other) = backend.filter(|b| *b != DPAPI) {
        return Err(format!("This profile's keys are protected by \"{other}\", which Windows can't open."));
    }
    let entropy = [ENTROPY_PREFIX, profile.as_bytes()].concat();
    let path = dir.join(FILE);
    match fs::read(&path) {
        Ok(blob) => {
            let plain = unprotect(&blob, &entropy).ok_or(
                "KeyHandoff's saved keys were changed or damaged, or come from another Windows account.",
            )?;
            let arr: [u8; 32] = plain[..].try_into().map_err(|_| "The stored master key is damaged.".to_string())?;
            Ok((MasterKey(Zeroizing::new(arr)), DPAPI))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if backend.is_some() {
                return Err("This profile's master key is missing, so its saved keys can't be opened.".into());
            }
            let mut key = Zeroizing::new([0u8; 32]);
            OsRng.fill_bytes(&mut key[..]);
            let blob = protect(&key[..], &entropy).ok_or("Windows could not protect Handoff's keys.")?;
            super::write_private(&path, &blob).map_err(|e| e.to_string())?;
            Ok((MasterKey(key), DPAPI))
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Delete the old copy once the key is in its slots.
pub fn forget_master_key(dir: &Path, _profile: &str, _backend: &str) -> Result<(), String> {
    fs::remove_file(dir.join(FILE)).map_err(|e| e.to_string())
}

fn blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
    // DPAPI only reads input blobs; the cast to *mut is what the struct wants.
    CRYPT_INTEGER_BLOB { cbData: bytes.len() as u32, pbData: bytes.as_ptr() as *mut u8 }
}

/// Take DPAPI's output buffer, optionally zero it, and free it.
fn take_output(out: CRYPT_INTEGER_BLOB, wipe: bool) -> Zeroizing<Vec<u8>> {
    // SAFETY: on success DPAPI hands us `cbData` bytes at `pbData`, allocated
    // with LocalAlloc, which we own and must free with LocalFree.
    unsafe {
        let data = std::slice::from_raw_parts_mut(out.pbData, out.cbData as usize);
        let copy = Zeroizing::new(data.to_vec());
        if wipe {
            data.zeroize();
        }
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut _)));
        copy
    }
}

fn protect(plain: &[u8], entropy: &[u8]) -> Option<Vec<u8>> {
    let (input, entropy) = (blob(plain), blob(entropy));
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY: all pointers are valid for the call; `out` is filled on success.
    unsafe { CryptProtectData(&input, None, Some(&entropy), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out) }.ok()?;
    Some(take_output(out, false).to_vec())
}

fn unprotect(sealed: &[u8], entropy: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    let (input, entropy) = (blob(sealed), blob(entropy));
    let mut out = CRYPT_INTEGER_BLOB::default();
    // SAFETY: as above.
    unsafe { CryptUnprotectData(&input, None, Some(&entropy), None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut out) }.ok()?;
    Some(take_output(out, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_key_back_and_wrong_profile_refused() {
        let dir = std::env::temp_dir().join(format!("handoff-protect-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (a, backend) = master_key(&dir, "test", None).unwrap();
        let (b, _) = master_key(&dir, "test", Some(backend)).unwrap();
        assert_eq!(a.0[..], b.0[..]);
        let blob = fs::read(dir.join(FILE)).unwrap();
        assert!(!blob.windows(32).any(|w| w == &a.0[..]), "master key readable on disk");
        assert!(master_key(&dir, "other", Some(DPAPI)).is_err(), "another profile's entropy opened it");
        fs::remove_dir_all(&dir).unwrap();
    }
}
