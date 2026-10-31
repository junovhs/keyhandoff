//! `keyslots.bin`: the profile's master key, wrapped once per way in. Each
//! slot is the master key sealed under a key derived from what you type
//! (your passphrase, or the recovery code), so a program reading the profile
//! folder gets nothing without one of them. Hardware slots (TPM PIN, Windows
//! Hello, a security key) add kinds later; any one slot opens the vault.
//!
//! Layout: `HOFK`, version, slot count, then per slot: kind, Argon2id memory
//! (KiB, u32 LE), passes (u32 LE), lanes, 16-byte salt, 12-byte nonce, and the
//! 48-byte sealed key. Everything before the sealed key is bound as AAD.

use std::fs;
use std::path::Path;

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand_core::{OsRng, RngCore};
use zeroize::Zeroizing;

use crate::os::{self, MasterKey};

pub const FILE: &str = "keyslots.bin";
const MAGIC: &[u8; 4] = b"HOFK";
const VERSION: u8 = 0;
const HEADER: usize = 1 + 4 + 4 + 1 + 16 + 12;
const SEALED: usize = 32 + 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Passphrase = 1,
    Recovery = 2,
}

/// Argon2id cost: memory in KiB, passes, lanes.
#[derive(Clone, Copy)]
struct Cost(u32, u32, u8);

/// RFC 9106's first choice is 2 GiB; 256 MiB keeps an unlock near a second
/// on an ordinary laptop while costing a guesser the same per try.
#[cfg(not(test))]
const PASSPHRASE: Cost = Cost(256 * 1024, 3, 4);
/// The recovery code is 160 random bits, so it needs no slow hash to resist
/// guessing; Argon2id here only keeps one code path.
#[cfg(not(test))]
const RECOVERY: Cost = Cost(19 * 1024, 2, 1);
#[cfg(test)]
const PASSPHRASE: Cost = Cost(64, 1, 1);
#[cfg(test)]
const RECOVERY: Cost = Cost(64, 1, 1);

/// Shortest passphrase accepted. A suggested one is five words.
pub const MIN_PASSPHRASE: usize = 12;

struct Slot {
    kind: Kind,
    cost: Cost,
    salt: [u8; 16],
    nonce: [u8; 12],
    sealed: [u8; SEALED],
}

impl Slot {
    fn seal(kind: Kind, cost: Cost, secret: &[u8], key: &MasterKey) -> Result<Self, String> {
        let mut salt = [0u8; 16];
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce);
        let mut slot = Self { kind, cost, salt, nonce, sealed: [0; SEALED] };
        let wrap = derive(cost, secret, &salt)?;
        let out = ChaCha20Poly1305::new(Key::from_slice(&wrap[..]))
            .encrypt(Nonce::from_slice(&nonce), Payload { msg: &key.0[..], aad: &slot.header() })
            .map_err(|_| "Could not protect the master key.".to_string())?;
        slot.sealed.copy_from_slice(&out);
        Ok(slot)
    }

    fn open(&self, secret: &[u8]) -> Result<Option<MasterKey>, String> {
        let wrap = derive(self.cost, secret, &self.salt)?;
        let Ok(plain) = ChaCha20Poly1305::new(Key::from_slice(&wrap[..]))
            .decrypt(Nonce::from_slice(&self.nonce), Payload { msg: &self.sealed, aad: &self.header() })
        else {
            return Ok(None);
        };
        let plain = Zeroizing::new(plain);
        let arr: [u8; 32] = plain[..].try_into().map_err(|_| "The key slots file is damaged.".to_string())?;
        Ok(Some(MasterKey(Zeroizing::new(arr))))
    }

    fn header(&self) -> Vec<u8> {
        let mut h = Vec::with_capacity(HEADER);
        h.push(self.kind as u8);
        h.extend_from_slice(&self.cost.0.to_le_bytes());
        h.extend_from_slice(&self.cost.1.to_le_bytes());
        h.push(self.cost.2);
        h.extend_from_slice(&self.salt);
        h.extend_from_slice(&self.nonce);
        h
    }
}

fn derive(cost: Cost, secret: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    let params = Params::new(cost.0, cost.1, cost.2 as u32, Some(32)).map_err(|e| e.to_string())?;
    let mut out = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(secret, salt, &mut out[..])
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// The slots of one profile.
pub struct Slots(Vec<Slot>);

impl Slots {
    pub fn exists(dir: &Path) -> bool {
        dir.join(FILE).exists()
    }

    pub fn load(dir: &Path) -> Result<Self, String> {
        let bytes = fs::read(dir.join(FILE)).map_err(|e| format!("The key slots file can't be read: {e}"))?;
        let damaged = || "The key slots file is damaged.".to_string();
        if bytes.len() < 6 || &bytes[..4] != MAGIC || bytes[4] != VERSION {
            return Err(damaged());
        }
        let count = bytes[5] as usize;
        let body = &bytes[6..];
        if body.len() != count * (HEADER + SEALED) {
            return Err(damaged());
        }
        let slots = body
            .chunks_exact(HEADER + SEALED)
            .map(|c| {
                let kind = match c[0] {
                    1 => Kind::Passphrase,
                    2 => Kind::Recovery,
                    _ => return Err(damaged()),
                };
                let u32_at = |i: usize| u32::from_le_bytes(c[i..i + 4].try_into().unwrap());
                Ok(Slot {
                    kind,
                    cost: Cost(u32_at(1), u32_at(5), c[9]),
                    salt: c[10..26].try_into().unwrap(),
                    nonce: c[26..38].try_into().unwrap(),
                    sealed: c[38..].try_into().unwrap(),
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self(slots))
    }

    /// New slots for `key`: one for the passphrase and one for a fresh
    /// recovery code, which is returned to show once.
    pub fn create(dir: &Path, key: &MasterKey, passphrase: &str) -> Result<Zeroizing<String>, String> {
        let passphrase = checked(passphrase)?;
        let code = recovery_code();
        let slots = Self(vec![
            Slot::seal(Kind::Passphrase, PASSPHRASE, passphrase.as_bytes(), key)?,
            Slot::seal(Kind::Recovery, RECOVERY, &code_bytes(&code).map_err(|_| "Could not make a recovery code.")?[..], key)?,
        ]);
        slots.save(dir)?;
        Ok(code)
    }

    /// The master key, if `passphrase` opens a passphrase slot.
    pub fn unlock(&self, passphrase: &str) -> Result<MasterKey, String> {
        self.open(Kind::Passphrase, passphrase.trim().as_bytes())?.ok_or_else(|| "That passphrase isn't right.".into())
    }

    /// Open with the recovery code and replace the passphrase.
    pub fn recover(&mut self, dir: &Path, code: &str, passphrase: &str) -> Result<MasterKey, String> {
        let passphrase = checked(passphrase)?;
        let bytes = code_bytes(code).map_err(|_| "That recovery code isn't right.".to_string())?;
        let key = self.open(Kind::Recovery, &bytes[..])?.ok_or("That recovery code isn't right.")?;
        self.0.retain(|s| s.kind != Kind::Passphrase);
        self.0.insert(0, Slot::seal(Kind::Passphrase, PASSPHRASE, passphrase.as_bytes(), &key)?);
        self.save(dir)?;
        Ok(key)
    }

    fn open(&self, kind: Kind, secret: &[u8]) -> Result<Option<MasterKey>, String> {
        for slot in self.0.iter().filter(|s| s.kind == kind) {
            if let Some(key) = slot.open(secret)? {
                return Ok(Some(key));
            }
        }
        Ok(None)
    }

    fn save(&self, dir: &Path) -> Result<(), String> {
        let mut out = Vec::with_capacity(6 + self.0.len() * (HEADER + SEALED));
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(self.0.len() as u8);
        for slot in &self.0 {
            out.extend_from_slice(&slot.header());
            out.extend_from_slice(&slot.sealed);
        }
        os::write_private(&dir.join(FILE), &out).map_err(|e| e.to_string())
    }
}

fn checked(passphrase: &str) -> Result<&str, String> {
    let p = passphrase.trim();
    if p.chars().count() < MIN_PASSPHRASE {
        return Err(format!("Use at least {MIN_PASSPHRASE} characters. Five random words is best."));
    }
    Ok(p)
}

/// Crockford base32: no I, L, O or U, so a code copied by hand reads back.
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// 160 random bits as eight groups of four, `7K2Q-…`.
fn recovery_code() -> Zeroizing<String> {
    let mut raw = Zeroizing::new([0u8; 20]);
    OsRng.fill_bytes(&mut raw[..]);
    let mut code = Zeroizing::new(String::with_capacity(39));
    for (i, chunk) in raw.chunks(5).enumerate() {
        let n = chunk.iter().fold(0u64, |acc, b| acc << 8 | *b as u64);
        for j in 0..8 {
            if (i * 8 + j) % 4 == 0 && i + j > 0 {
                code.push('-');
            }
            code.push(CROCKFORD[(n >> (35 - 5 * j) & 31) as usize] as char);
        }
    }
    code
}

/// A typed recovery code as its 32 symbols, forgiving case, dashes, spaces
/// and the letters people write for 0 and 1.
fn code_bytes(code: &str) -> Result<Zeroizing<Vec<u8>>, ()> {
    let mut out = Zeroizing::new(Vec::with_capacity(32));
    for c in code.chars().filter(|c| !c.is_whitespace() && *c != '-') {
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        };
        if !CROCKFORD.contains(&(c as u8)) || !c.is_ascii() {
            return Err(());
        }
        out.push(c as u8);
    }
    if out.len() != 32 {
        return Err(());
    }
    Ok(out)
}

/// Five words from the EFF long list (about 64 bits), to suggest as a passphrase.
pub fn suggest() -> Zeroizing<String> {
    static WORDS: &str = include_str!("../assets/eff_large_wordlist.txt");
    let words: Vec<&str> = WORDS.lines().collect();
    let mut out = Zeroizing::new(String::new());
    for i in 0..5 {
        // Rejection sampling keeps every word equally likely.
        let zone = u32::MAX - u32::MAX % words.len() as u32;
        let n = loop {
            let n = OsRng.next_u32();
            if n < zone {
                break n;
            }
        };
        if i > 0 {
            out.push(' ');
        }
        out.push_str(words[n as usize % words.len()]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_codes_read_back_however_they_are_typed() {
        let code = recovery_code();
        assert_eq!(code.len(), 39);
        assert_eq!(code.matches('-').count(), 7);
        let sloppy = code.to_lowercase().replace('-', " ").replace('0', "o").replace('1', "l");
        assert_eq!(code_bytes(&sloppy).unwrap(), code_bytes(&code).unwrap());
        assert!(code_bytes("short").is_err());
        assert_eq!(suggest().split(' ').count(), 5);
    }
}
