//! Key requests: "this computer is asking for a key", carrying a fresh
//! one-time public key and signed by the requester's identity.

use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::identity::{Identity, PublicIdentity};
use crate::keyring::PendingRequest;
use crate::wire::{self, Reader};
use crate::{Error, Kind, MAX_NAME_BYTES};

pub(crate) const SIGN_DOMAIN: &[u8] = b"handoff:request:v0";

/// A verified request. Holds no secret; safe to send anywhere.
#[derive(Clone, Debug)]
pub struct Request {
    bytes: Vec<u8>,
    id: [u8; 16],
    created_at: u64,
    expires_at: u64,
    requester: PublicIdentity,
    one_time: [u8; 32],
    name: String,
}

/// Make a request from this computer. Returns the request to send and the
/// pending entry (with the one-time private key) to keep in the [`Keyring`].
///
/// [`Keyring`]: crate::Keyring
pub fn create_request(
    identity: &Identity,
    name: &str,
    now: u64,
    ttl: u64,
) -> Result<(Request, PendingRequest), Error> {
    if !wire::valid_text(name, MAX_NAME_BYTES, false) || ttl == 0 {
        return Err(Error::InvalidField);
    }
    let expires_at = now.checked_add(ttl).ok_or(Error::InvalidField)?;
    let mut id = [0u8; 16];
    OsRng.try_fill_bytes(&mut id).map_err(|_| Error::Randomness)?;
    let secret = StaticSecret::random_from_rng(OsRng);
    let one_time = PublicKey::from(&secret).to_bytes();

    let mut bytes = Vec::with_capacity(170 + name.len());
    wire::write_header(&mut bytes, Kind::Request);
    bytes.extend_from_slice(&id);
    bytes.extend_from_slice(&now.to_be_bytes());
    bytes.extend_from_slice(&expires_at.to_be_bytes());
    bytes.extend_from_slice(identity.public().as_bytes());
    bytes.extend_from_slice(&one_time);
    bytes.push(name.len() as u8);
    bytes.extend_from_slice(name.as_bytes());
    let signature = identity.sign(SIGN_DOMAIN, &bytes);
    bytes.extend_from_slice(&signature);

    let request = Request::parse(&bytes)?;
    let pending = PendingRequest::new(id, secret, expires_at, request.digest());
    Ok((request, pending))
}

impl Request {
    /// Parse a request and verify its signature. Expiry is checked when sealing.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if wire::header(bytes)? != Kind::Request {
            return Err(Error::WrongKind);
        }
        let mut r = Reader::new(bytes, 5);
        let id = r.array()?;
        let created_at = r.u64()?;
        let expires_at = r.u64()?;
        let requester = PublicIdentity::from_bytes(r.array()?)?;
        let one_time = r.array()?;
        let name_len = r.u8()? as usize;
        let name = r.text(name_len, MAX_NAME_BYTES, false)?;
        let signed_len = r.pos();
        let signature = r.array()?;
        r.finish()?;
        if expires_at <= created_at {
            return Err(Error::Malformed);
        }
        requester.verify(SIGN_DOMAIN, &bytes[..signed_len], &signature)?;
        Ok(Self { bytes: bytes.to_vec(), id, created_at, expires_at, requester, one_time, name })
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn id(&self) -> [u8; 16] {
        self.id
    }

    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub fn requester(&self) -> PublicIdentity {
        self.requester
    }

    /// The requester's display name, as they typed it. Not verified by anyone.
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_expired(&self, now: u64) -> bool {
        now >= self.expires_at
    }

    pub(crate) fn one_time_key(&self) -> &[u8; 32] {
        &self.one_time
    }

    /// SHA-256 of the whole signed request; salts the sealing key derivation.
    pub(crate) fn digest(&self) -> [u8; 32] {
        Sha256::digest(&self.bytes).into()
    }
}
