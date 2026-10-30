//! The requester's pending one-time keys. Opening a sealed key consumes its
//! entry, so every copy of that sealed key is unopenable afterwards.

use x25519_dalek::StaticSecret;
use zeroize::{Zeroize, Zeroizing};

use crate::identity::PublicIdentity;
use crate::sealed::{Payload, Sealed};
use crate::wire::Reader;
use crate::Error;

const KEYRING_MAGIC: &[u8; 4] = b"HOFK";
const ENTRY_BYTES: usize = 16 + 32 + 8 + 32;

/// One request's one-time private key and what it is bound to.
/// The key is zeroized on drop.
pub struct PendingRequest {
    id: [u8; 16],
    secret: StaticSecret,
    expires_at: u64,
    request_digest: [u8; 32],
}

impl PendingRequest {
    pub(crate) fn new(id: [u8; 16], secret: StaticSecret, expires_at: u64, request_digest: [u8; 32]) -> Self {
        Self { id, secret, expires_at, request_digest }
    }

    pub fn id(&self) -> [u8; 16] {
        self.id
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }
}

/// What opening a sealed key gives the receiver.
#[derive(Debug)]
pub struct Opened {
    pub payload: Payload,
    pub sender: PublicIdentity,
}

/// All pending one-time keys on this computer. Persist with
/// [`Keyring::to_bytes`] under OS protection after every change.
#[derive(Default)]
pub struct Keyring {
    pending: Vec<PendingRequest>,
}

impl Keyring {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, pending: PendingRequest) {
        self.destroy(pending.id);
        self.pending.push(pending);
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn contains(&self, id: [u8; 16]) -> bool {
        self.pending.iter().any(|p| p.id == id)
    }

    /// Open a sealed key and destroy its one-time key. A sealed key for a
    /// different request, or one that fails to decrypt, leaves the keyring
    /// untouched. An expired entry is destroyed and refused.
    pub fn open(&mut self, sealed: &Sealed, now: u64) -> Result<Opened, Error> {
        let index = self.pending.iter().position(|p| p.id == sealed.request_id()).ok_or(Error::UnknownRequest)?;
        if now >= self.pending[index].expires_at {
            self.pending.swap_remove(index);
            return Err(Error::Expired);
        }
        let entry = &self.pending[index];
        let payload = sealed.open_with(&entry.secret, &entry.request_digest)?;
        self.pending.swap_remove(index);
        Ok(Opened { payload, sender: sealed.sender() })
    }

    /// Withdraw a request. Returns whether it was pending.
    pub fn destroy(&mut self, id: [u8; 16]) -> bool {
        let before = self.pending.len();
        self.pending.retain(|p| p.id != id);
        self.pending.len() != before
    }

    /// Destroy every entry that has run out. Returns how many were removed.
    pub fn purge_expired(&mut self, now: u64) -> usize {
        let before = self.pending.len();
        self.pending.retain(|p| now < p.expires_at);
        before - self.pending.len()
    }

    /// Serialize for storage. The output holds private keys: wrap it with the
    /// OS key protection before writing it anywhere.
    pub fn to_bytes(&self) -> Zeroizing<Vec<u8>> {
        let mut out = Zeroizing::new(Vec::with_capacity(8 + self.pending.len() * ENTRY_BYTES));
        out.extend_from_slice(KEYRING_MAGIC);
        out.extend_from_slice(&(self.pending.len() as u32).to_be_bytes());
        for p in &self.pending {
            out.extend_from_slice(&p.id);
            out.extend_from_slice(p.secret.as_bytes());
            out.extend_from_slice(&p.expires_at.to_be_bytes());
            out.extend_from_slice(&p.request_digest);
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let mut r = Reader::new(bytes, 0);
        if r.take(4)? != KEYRING_MAGIC {
            return Err(Error::Malformed);
        }
        let count = u32::from_be_bytes(r.array()?) as usize;
        if bytes.len() != 8 + count.checked_mul(ENTRY_BYTES).ok_or(Error::Malformed)? {
            return Err(Error::Malformed);
        }
        let mut pending = Vec::with_capacity(count);
        for _ in 0..count {
            let id = r.array()?;
            let mut raw = r.array::<32>()?;
            let secret = StaticSecret::from(raw);
            raw.zeroize();
            let expires_at = r.u64()?;
            let request_digest = r.array()?;
            pending.push(PendingRequest { id, secret, expires_at, request_digest });
        }
        r.finish()?;
        Ok(Self { pending })
    }
}

impl std::fmt::Debug for PendingRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingRequest").field("expires_at", &self.expires_at).finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Keyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Keyring({} pending)", self.pending.len())
    }
}
