//! Kept keys. Each secret is sealed with ChaCha20-Poly1305
//! under a random vault key, with the item id as associated data, so a loaded
//! vault holds ciphertext until a key is used (docs/sealing-format.md).
//! The vault key and items are written together as one OS-protected file.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use keyhandoff_seal::Secret;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const DAY: u64 = 24 * 60 * 60;
/// How many accepted request ids are remembered, to explain a second open.
const OPENED_MEMORY: usize = 500;

/// How long the receiver keeps a key. The last choice is the next default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Keep {
    /// Deleted after its first use by a program.
    Once,
    Days30,
    #[default]
    #[serde(rename = "keep")]
    Forever,
}

impl Keep {
    fn expires_at(self, from: u64) -> Option<u64> {
        match self {
            Keep::Days30 => Some(from + 30 * DAY),
            Keep::Once | Keep::Forever => None,
        }
    }
}

/// A kept key: name, source and dates are readable; the value is sealed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: [u8; 16],
    pub label: String,
    /// The sender's display name; `None` when you added it yourself.
    pub from: Option<String>,
    /// The sender's identity fingerprint.
    pub sender: Option<String>,
    pub added_at: u64,
    pub keep: Keep,
    pub expires_at: Option<u64>,
    /// The team it was shared with, if any.
    #[serde(default)]
    pub team: Option<String>,
    nonce: [u8; 12],
    sealed: Vec<u8>,
}

#[derive(Serialize, Deserialize, Default)]
struct Items {
    items: Vec<Item>,
    opened: Vec<[u8; 16]>,
}

pub struct Vault {
    key: Zeroizing<[u8; 32]>,
    items: Vec<Item>,
    /// Request ids whose sealed key was already accepted here.
    opened: Vec<[u8; 16]>,
}

impl Vault {
    pub fn new() -> Self {
        let mut key = Zeroizing::new([0u8; 32]);
        OsRng.fill_bytes(&mut key[..]);
        Self { key, items: Vec::new(), opened: Vec::new() }
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// `label`, or `label 2`, `label 3`… when a kept key already has that
    /// name. Names compare case-insensitively after trimming, so a
    /// `handoff:Name` pointer always names exactly one key.
    pub fn free_name(&self, label: &str) -> String {
        let base = label.trim();
        let taken = |name: &str| self.items.iter().any(|i| same_name(&i.label, name));
        if !taken(base) {
            return base.to_string();
        }
        (2..).map(|n| format!("{base} {n}")).find(|name| !taken(name)).expect("some numbered name is free")
    }

    /// The kept key a `handoff:Name` pointer names.
    pub fn find(&self, name: &str) -> Option<[u8; 16]> {
        self.items.iter().find(|i| same_name(&i.label, name)).map(|i| i.id)
    }

    /// Seal and store a secret under a free name. Returns the new item's id.
    pub fn add(&mut self, label: &str, from: Option<String>, sender: Option<String>, secret: &Secret, keep: Keep, now: u64) -> [u8; 16] {
        let mut id = [0u8; 16];
        OsRng.fill_bytes(&mut id);
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let label = self.free_name(label);
        let sealed = self
            .cipher()
            .encrypt(Nonce::from_slice(&nonce), Payload { msg: secret.expose(), aad: &id })
            .expect("ChaCha20-Poly1305 encryption cannot fail for in-memory data");
        self.items.push(Item {
            id,
            label,
            from,
            sender,
            added_at: now,
            keep,
            expires_at: keep.expires_at(now),
            team: None,
            nonce,
            sealed,
        });
        id
    }

    /// Put a new value in an existing item: same id, so programs allowed to
    /// use it still can, and same name, so `.env` pointers still find it.
    pub fn replace(&mut self, id: [u8; 16], secret: &Secret) -> bool {
        let cipher = self.cipher();
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else { return false };
        OsRng.fill_bytes(&mut item.nonce);
        item.sealed = cipher
            .encrypt(Nonce::from_slice(&item.nonce), Payload { msg: secret.expose(), aad: &item.id })
            .expect("ChaCha20-Poly1305 encryption cannot fail for in-memory data");
        true
    }

    /// File a kept key under a team.
    pub fn set_team(&mut self, id: [u8; 16], team: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.team = Some(team.to_string());
        }
    }

    /// Change how long an item is kept, counted from when it was added.
    pub fn set_keep(&mut self, id: [u8; 16], keep: Keep) -> bool {
        let Some(item) = self.items.iter_mut().find(|i| i.id == id) else { return false };
        item.keep = keep;
        item.expires_at = keep.expires_at(item.added_at);
        true
    }

    pub fn remove(&mut self, id: [u8; 16]) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() != before
    }

    /// Delete everything that ran out. Returns how many were removed.
    pub fn purge(&mut self, now: u64) -> usize {
        let before = self.items.len();
        self.items.retain(|i| i.expires_at.is_none_or(|t| now < t));
        before - self.items.len()
    }

    /// Decrypt one kept key for use.
    pub fn secret(&self, id: [u8; 16]) -> Option<Secret> {
        let item = self.items.iter().find(|i| i.id == id)?;
        let plain = self
            .cipher()
            .decrypt(Nonce::from_slice(&item.nonce), Payload { msg: &item.sealed, aad: &item.id })
            .ok()?;
        Some(Secret::new(plain))
    }

    pub fn mark_opened(&mut self, request_id: [u8; 16]) {
        self.opened.push(request_id);
        if self.opened.len() > OPENED_MEMORY {
            self.opened.remove(0);
        }
    }

    pub fn was_opened(&self, request_id: [u8; 16]) -> bool {
        self.opened.contains(&request_id)
    }

    fn cipher(&self) -> ChaCha20Poly1305 {
        ChaCha20Poly1305::new(Key::from_slice(&self.key[..]))
    }

    /// Vault key then JSON metadata. Holds the vault key: protect before writing.
    pub fn to_bytes(&self) -> Zeroizing<Vec<u8>> {
        let items = Items { items: self.items.clone(), opened: self.opened.clone() };
        let mut out = Zeroizing::new(self.key.to_vec());
        serde_json::to_writer(&mut *out, &items).expect("vault items serialize");
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 32 {
            return Err("The vault file is damaged.".into());
        }
        let key: [u8; 32] = bytes[..32].try_into().expect("checked length");
        let items: Items = serde_json::from_slice(&bytes[32..]).map_err(|_| "The vault file is damaged.".to_string())?;
        let mut vault = Self { key: Zeroizing::new(key), items: Vec::new(), opened: items.opened };
        // Vaults saved before names were unique can hold duplicates.
        for mut item in items.items {
            item.label = vault.free_name(&item.label);
            vault.items.push(item);
        }
        Ok(vault)
    }
}

fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn secret() -> Secret {
        Secret::new(b"sk-test-synthetic-0001".to_vec())
    }

    #[test]
    fn stores_sealed_and_round_trips() {
        let mut v = Vault::new();
        let id = v.add("Dialpad", Some("Ben".into()), None, &secret(), Keep::Forever, NOW);
        let bytes = v.to_bytes();
        assert!(!bytes.windows(22).any(|w| w == b"sk-test-synthetic-0001"), "plaintext in vault file");
        let back = Vault::from_bytes(&bytes).unwrap();
        assert_eq!(back.items()[0].label, "Dialpad");
        assert_eq!(back.secret(id).unwrap().expose(), b"sk-test-synthetic-0001");
    }

    #[test]
    fn a_thousand_keys_round_trip() {
        let mut v = Vault::new();
        let ids: Vec<_> = (0..1000).map(|i| v.add(&format!("Key {i}"), None, None, &secret(), Keep::Forever, NOW)).collect();
        let back = Vault::from_bytes(&v.to_bytes()).unwrap();
        assert_eq!(back.items().len(), 1000);
        assert_eq!(back.items()[999].label, "Key 999");
        assert_eq!(back.secret(ids[999]).unwrap().expose(), b"sk-test-synthetic-0001");
    }

    #[test]
    fn names_are_unique_so_each_pointer_finds_its_own_key() {
        let mut v = Vault::new();
        let a = v.add("Stripe live", None, None, &secret(), Keep::Forever, NOW);
        let b = v.add("Stripe live", None, None, &Secret::new(b"sk-test-synthetic-0002".to_vec()), Keep::Forever, NOW);
        let c = v.add(" stripe LIVE ", None, None, &Secret::new(b"sk-test-synthetic-0003".to_vec()), Keep::Forever, NOW);
        let names: Vec<_> = v.items().iter().map(|i| i.label.as_str()).collect();
        assert_eq!(names, ["Stripe live", "Stripe live 2", "stripe LIVE 3"]);
        for (name, id, value) in [("Stripe live", a, "0001"), ("stripe live 2", b, "0002"), ("Stripe live 3", c, "0003")] {
            assert_eq!(v.find(name), Some(id));
            assert_eq!(v.secret(id).unwrap().expose(), format!("sk-test-synthetic-{value}").as_bytes());
        }

        // A vault saved with duplicates loads with distinct names.
        v.items[1].label = "Stripe live".into();
        let back = Vault::from_bytes(&v.to_bytes()).unwrap();
        assert_eq!(back.find("Stripe live 2"), Some(b));
    }

    #[test]
    fn swapped_ciphertext_does_not_open() {
        let mut v = Vault::new();
        let a = v.add("A", None, None, &secret(), Keep::Forever, NOW);
        let b = v.add("B", None, None, &Secret::new(b"other".to_vec()), Keep::Forever, NOW);
        let sealed_b = v.items[1].sealed.clone();
        let nonce_b = v.items[1].nonce;
        v.items[0].sealed = sealed_b;
        v.items[0].nonce = nonce_b;
        assert!(v.secret(a).is_none(), "item id is bound as associated data");
        assert!(v.secret(b).is_some());
    }

    #[test]
    fn thirty_days_expire_and_keep_changes_apply() {
        let mut v = Vault::new();
        let id = v.add("Temp", None, None, &secret(), Keep::Days30, NOW);
        v.add("Forever", None, None, &secret(), Keep::Forever, NOW);
        assert_eq!(v.purge(NOW + 29 * DAY), 0);
        assert_eq!(v.purge(NOW + 30 * DAY), 1);
        assert_eq!(v.items()[0].label, "Forever");

        let id2 = v.add("Later", None, None, &secret(), Keep::Forever, NOW);
        assert!(v.set_keep(id2, Keep::Days30));
        assert_eq!(v.items().iter().find(|i| i.id == id2).unwrap().expires_at, Some(NOW + 30 * DAY));
        assert!(!v.set_keep(id, Keep::Forever), "purged item is gone");
    }

    #[test]
    fn opened_requests_are_remembered_and_bounded() {
        let mut v = Vault::new();
        v.mark_opened([1; 16]);
        assert!(v.was_opened([1; 16]));
        for i in 0..OPENED_MEMORY {
            v.mark_opened([(i % 250) as u8 + 2; 16]);
        }
        assert!(!v.was_opened([1; 16]));
        let back = Vault::from_bytes(&v.to_bytes()).unwrap();
        assert_eq!(back.opened.len(), OPENED_MEMORY);
    }
}
