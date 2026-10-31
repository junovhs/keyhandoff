//! A profile's files in the app data folder. The identity, the pending
//! one-time keys, the vault, the programs you allowed and your contacts are sealed under the OS-protected master key
//! (sealing-adr.md: DPAPI on Windows; the Secret Service on Linux for now).
//! `settings.json` holds no secret.
//!
//! Profiles let two identities share one computer for testing
//! (`--profile ben` or `KEYHANDOFF_PROFILE=ben`).

use std::fs;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use keyhandoff_seal::{Identity, Keyring};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::os::{self, MasterKey};
use crate::vault::{Keep, Vault};

const MAGIC: &[u8; 4] = b"HOFP";
const VERSION: u8 = 0;
const IDENTITY: &str = "identity.bin";
const PENDING: &str = "pending.bin";
const VAULT: &str = "vault.bin";
const APPROVALS: &str = "approvals.bin";
const CONTACTS: &str = "contacts.bin";
const SETTINGS: &str = "settings.json";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    /// Your display name, shown on your requests.
    pub name: String,
    /// The last keep-time chosen; the next default.
    #[serde(default)]
    pub keep: Keep,
    /// Which OS backend holds the master key, fixed on first run.
    #[serde(default)]
    pub protection: Option<String>,
}

/// A program allowed to get one kept key when it runs in one folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub program: PathBuf,
    pub folder: Option<PathBuf>,
    pub key: [u8; 16],
}

/// Someone whose card you opened: their computer and the locks you can still
/// seal a key to, each used once.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contact {
    pub name: String,
    pub identity: [u8; 32],
    /// Requests from their card, as sent.
    pub locks: Vec<Vec<u8>>,
}

pub struct Store {
    dir: PathBuf,
    key: MasterKey,
}

/// A profile name: 1-32 of `a-z 0-9 - _`.
pub fn valid_profile(name: &str) -> bool {
    !name.is_empty() && name.len() <= 32 && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

pub fn profile_dir(profile: &str) -> PathBuf {
    // Keeps its pre-rename name so existing profiles stay where they are.
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("handoff").join(profile)
}

impl Store {
    /// Open (or create) a profile, fetching its master key from the OS.
    pub fn open(profile: &str) -> Result<(Self, Settings), String> {
        let dir = profile_dir(profile);
        os::private_dir(&dir).map_err(|e| e.to_string())?;
        let mut settings = read_settings(&dir).unwrap_or_default();
        let (key, backend) = os::master_key(&dir, profile, settings.protection.as_deref())?;
        if settings.name.is_empty() {
            settings.name = default_name(profile);
        }
        let store = Self { dir, key };
        if settings.protection.as_deref() != Some(backend) {
            settings.protection = Some(backend.to_string());
        }
        store.save_settings(&settings)?;
        Ok((store, settings))
    }

    #[cfg(test)]
    pub fn in_dir(dir: &Path, key: MasterKey) -> Self {
        Self { dir: dir.to_path_buf(), key }
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
        os::write_private(&self.dir.join(SETTINGS), &json).map_err(|e| e.to_string())
    }

    /// This computer's identity, created on first use.
    pub fn identity(&self) -> Result<Identity, String> {
        if let Some(bytes) = self.read(IDENTITY)? {
            let arr: [u8; 32] = bytes[..].try_into().map_err(|_| "The identity file is damaged.".to_string())?;
            return Ok(Identity::from_bytes(&Zeroizing::new(arr)));
        }
        let identity = Identity::generate();
        self.write(IDENTITY, &identity.to_bytes()[..])?;
        Ok(identity)
    }

    pub fn keyring(&self) -> Result<Keyring, String> {
        match self.read(PENDING)? {
            Some(bytes) => Keyring::from_bytes(&bytes).map_err(|_| "The pending requests file is damaged.".to_string()),
            None => Ok(Keyring::new()),
        }
    }

    pub fn save_keyring(&self, keyring: &Keyring) -> Result<(), String> {
        self.write(PENDING, &keyring.to_bytes())
    }

    pub fn vault(&self) -> Result<Vault, String> {
        match self.read(VAULT)? {
            Some(bytes) => Vault::from_bytes(&bytes),
            None => Ok(Vault::new()),
        }
    }

    pub fn save_vault(&self, vault: &Vault) -> Result<(), String> {
        self.write(VAULT, &vault.to_bytes())
    }

    pub fn approvals(&self) -> Result<Vec<Approval>, String> {
        match self.read(APPROVALS)? {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|_| "The allowed programs file is damaged.".to_string()),
            None => Ok(Vec::new()),
        }
    }

    pub fn save_approvals(&self, approvals: &[Approval]) -> Result<(), String> {
        let json = serde_json::to_vec(approvals).map_err(|e| e.to_string())?;
        self.write(APPROVALS, &json)
    }

    pub fn contacts(&self) -> Result<Vec<Contact>, String> {
        match self.read(CONTACTS)? {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|_| "The contacts file is damaged.".to_string()),
            None => Ok(Vec::new()),
        }
    }

    pub fn save_contacts(&self, contacts: &[Contact]) -> Result<(), String> {
        let json = serde_json::to_vec(contacts).map_err(|e| e.to_string())?;
        self.write(CONTACTS, &json)
    }

    fn cipher(&self) -> ChaCha20Poly1305 {
        ChaCha20Poly1305::new(Key::from_slice(&self.key.0[..]))
    }

    fn write(&self, name: &str, plain: &[u8]) -> Result<(), String> {
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let aad = aad(name);
        let sealed = self
            .cipher()
            .encrypt(Nonce::from_slice(&nonce), Payload { msg: plain, aad: &aad })
            .map_err(|_| "Could not protect a file.".to_string())?;
        let mut out = Vec::with_capacity(MAGIC.len() + 1 + nonce.len() + sealed.len());
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&sealed);
        os::write_private(&self.dir.join(name), &out).map_err(|e| e.to_string())
    }

    fn read(&self, name: &str) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        let bytes = match fs::read(self.dir.join(name)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let damaged = || format!("{name} is damaged or was protected by a different sign-in.");
        if bytes.len() < 17 || &bytes[..4] != MAGIC || bytes[4] != VERSION {
            return Err(damaged());
        }
        let aad = aad(name);
        let plain = self
            .cipher()
            .decrypt(Nonce::from_slice(&bytes[5..17]), Payload { msg: &bytes[17..], aad: &aad })
            .map_err(|_| damaged())?;
        Ok(Some(Zeroizing::new(plain)))
    }
}

fn aad(name: &str) -> Vec<u8> {
    [b"handoff:file:v0:".as_slice(), name.as_bytes()].concat()
}

fn read_settings(dir: &Path) -> Option<Settings> {
    serde_json::from_slice(&fs::read(dir.join(SETTINGS)).ok()?).ok()
}

/// "ben" becomes "Ben"; the default profile uses the login name.
fn default_name(profile: &str) -> String {
    let base = if profile == "default" { os::user_name().unwrap_or_else(|| "Me".into()) } else { profile.to_string() };
    let mut chars = base.chars();
    let name: String = chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default();
    name.chars().filter(|c| !c.is_control()).take(32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyhandoff_seal::Secret;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut n = [0u8; 8];
        OsRng.fill_bytes(&mut n);
        let dir = std::env::temp_dir().join(format!("handoff-test-{tag}-{}", u64::from_le_bytes(n)));
        os::private_dir(&dir).unwrap();
        dir
    }

    #[test]
    fn protected_files_hold_no_plaintext_and_need_the_key() {
        let dir = temp_dir("store");
        let store = Store::in_dir(&dir, MasterKey::for_tests());
        let identity = store.identity().unwrap();
        assert_eq!(store.identity().unwrap().public(), identity.public(), "identity persists");

        let mut vault = store.vault().unwrap();
        vault.add("Dialpad", None, None, &Secret::new(b"sk-synthetic-store".to_vec()), Keep::Forever, 1);
        store.save_vault(&vault).unwrap();
        for name in [IDENTITY, VAULT] {
            let raw = fs::read(dir.join(name)).unwrap();
            assert!(!raw.windows(7).any(|w| w == b"Dialpad"), "{name} leaks metadata");
            assert!(!raw.windows(18).any(|w| w == b"sk-synthetic-store"));
        }
        assert!(!fs::read(dir.join(IDENTITY)).unwrap().windows(32).any(|w| w == &identity.to_bytes()[..]));

        let other = Store::in_dir(&dir, MasterKey::for_tests());
        assert!(other.vault().is_err(), "a different master key cannot open the vault");

        // A file renamed onto another slot does not open (name is bound).
        fs::copy(dir.join(VAULT), dir.join(PENDING)).unwrap();
        assert!(store.keyring().is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn profile_names_and_default_display_names() {
        assert!(valid_profile("ben") && valid_profile("a-1_b"));
        assert!(!valid_profile("") && !valid_profile("../x") && !valid_profile("Ben"));
        assert_eq!(default_name("ben"), "Ben");
    }
}
