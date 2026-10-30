//! Sealed keys: a secret encrypted to one request's one-time key and signed
//! by the sender's identity.

use chacha20poly1305::aead::{Aead, KeyInit, Payload as AeadPayload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use std::fmt;
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::identity::{Identity, PublicIdentity};
use crate::request::Request;
use crate::wire::{self, Reader};
use crate::{Error, Kind, MAX_LABEL_BYTES, MAX_NOTE_BYTES, MAX_SECRET_BYTES};

const SIGN_DOMAIN: &[u8] = b"handoff:sealed:v0";
const KDF_INFO: &[u8] = b"handoff:seal:v0";
const TAG_BYTES: usize = 16;
/// Header bytes before the nonce; they are the AEAD associated data.
const AAD_END: usize = 5 + 16 + 32 + 32;

/// Secret bytes. Zeroized on drop and never printed by `Debug`.
pub struct Secret(Zeroizing<Vec<u8>>);

impl Secret {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// Only for handing the value to the vault, a typed box or a program.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret({} bytes)", self.0.len())
    }
}

/// What travels inside a sealed key. The label is encrypted too.
#[derive(Debug)]
pub struct Payload {
    pub label: String,
    pub note: String,
    pub secret: Secret,
}

impl Payload {
    fn encode(&self) -> Result<Zeroizing<Vec<u8>>, Error> {
        let secret = self.secret.expose();
        if !wire::valid_text(&self.label, MAX_LABEL_BYTES, false)
            || !wire::valid_text(&self.note, MAX_NOTE_BYTES, true)
            || secret.is_empty()
            || secret.len() > MAX_SECRET_BYTES
        {
            return Err(Error::InvalidField);
        }
        let mut out = Zeroizing::new(Vec::with_capacity(4 + self.label.len() + self.note.len() + secret.len()));
        out.push(self.label.len() as u8);
        out.extend_from_slice(self.label.as_bytes());
        out.push(self.note.len() as u8);
        out.extend_from_slice(self.note.as_bytes());
        out.extend_from_slice(&(secret.len() as u16).to_be_bytes());
        out.extend_from_slice(secret);
        Ok(out)
    }

    fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let mut r = Reader::new(bytes, 0);
        let label_len = r.u8()? as usize;
        let label = r.text(label_len, MAX_LABEL_BYTES, false)?;
        let note_len = r.u8()? as usize;
        let note = r.text(note_len, MAX_NOTE_BYTES, true)?;
        let secret_len = r.u16()? as usize;
        if secret_len == 0 || secret_len > MAX_SECRET_BYTES {
            return Err(Error::Malformed);
        }
        let secret = Secret::new(r.take(secret_len)?.to_vec());
        r.finish()?;
        Ok(Self { label, note, secret })
    }
}

/// A verified sealed key. Safe to send anywhere: only the request's one-time
/// key opens it.
#[derive(Clone, Debug)]
pub struct Sealed {
    bytes: Vec<u8>,
    request_id: [u8; 16],
    ephemeral: [u8; 32],
    sender: PublicIdentity,
    nonce: [u8; 12],
    ciphertext_range: (usize, usize),
}

/// Seal `payload` to `request`, signed by `sender`. Refuses expired requests.
pub fn seal(sender: &Identity, request: &Request, payload: &Payload, now: u64) -> Result<Sealed, Error> {
    if request.is_expired(now) {
        return Err(Error::Expired);
    }
    let plaintext = payload.encode()?;

    let ephemeral = EphemeralSecret::random_from_rng(OsRng);
    let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
    let one_time = PublicKey::from(*request.one_time_key());
    let shared = ephemeral.diffie_hellman(&one_time);
    if !shared.was_contributory() {
        return Err(Error::Malformed);
    }
    let key = derive_key(shared.as_bytes(), &request.digest(), &ephemeral_public, one_time.as_bytes());

    let mut bytes = Vec::with_capacity(AAD_END + 14 + plaintext.len() + TAG_BYTES + 64);
    wire::write_header(&mut bytes, Kind::Sealed);
    bytes.extend_from_slice(&request.id());
    bytes.extend_from_slice(&ephemeral_public);
    bytes.extend_from_slice(sender.public().as_bytes());
    let mut nonce = [0u8; 12];
    OsRng.try_fill_bytes(&mut nonce).map_err(|_| Error::Randomness)?;
    let ciphertext = ChaCha20Poly1305::new(Key::from_slice(&key[..]))
        .encrypt(Nonce::from_slice(&nonce), AeadPayload { msg: &plaintext, aad: &bytes[..AAD_END] })
        .map_err(|_| Error::Malformed)?;
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&(ciphertext.len() as u16).to_be_bytes());
    bytes.extend_from_slice(&ciphertext);
    let signature = sender.sign(SIGN_DOMAIN, &bytes);
    bytes.extend_from_slice(&signature);

    Sealed::parse(&bytes)
}

fn derive_key(shared: &[u8; 32], request_digest: &[u8; 32], ephemeral: &[u8; 32], one_time: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(Some(request_digest), shared)
        .expand_multi_info(&[KDF_INFO, ephemeral, one_time], &mut key[..])
        .expect("32 bytes is a valid HKDF-SHA256 length");
    key
}

impl Sealed {
    /// Parse a sealed key and verify the sender's signature.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if wire::header(bytes)? != Kind::Sealed {
            return Err(Error::WrongKind);
        }
        let mut r = Reader::new(bytes, 5);
        let request_id = r.array()?;
        let ephemeral = r.array()?;
        let sender = PublicIdentity::from_bytes(r.array()?)?;
        let nonce = r.array()?;
        let ciphertext_len = r.u16()? as usize;
        let start = r.pos();
        if ciphertext_len < TAG_BYTES {
            return Err(Error::Malformed);
        }
        r.take(ciphertext_len)?;
        let signed_len = r.pos();
        let signature = r.array()?;
        r.finish()?;
        sender.verify(SIGN_DOMAIN, &bytes[..signed_len], &signature)?;
        Ok(Self { bytes: bytes.to_vec(), request_id, ephemeral, sender, nonce, ciphertext_range: (start, signed_len) })
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn request_id(&self) -> [u8; 16] {
        self.request_id
    }

    /// The computer that sealed it (verified signature; whose it is is the
    /// app's continuity check).
    pub fn sender(&self) -> PublicIdentity {
        self.sender
    }

    /// Decrypt with a one-time key. Used by the keyring, which also enforces
    /// expiry and destroys the key afterwards.
    pub(crate) fn open_with(&self, one_time: &StaticSecret, request_digest: &[u8; 32]) -> Result<Payload, Error> {
        let one_time_public = PublicKey::from(one_time).to_bytes();
        let shared = one_time.diffie_hellman(&PublicKey::from(self.ephemeral));
        if !shared.was_contributory() {
            return Err(Error::DecryptFailed);
        }
        let key = derive_key(shared.as_bytes(), request_digest, &self.ephemeral, &one_time_public);
        let (start, end) = self.ciphertext_range;
        let plaintext = Zeroizing::new(
            ChaCha20Poly1305::new(Key::from_slice(&key[..]))
                .decrypt(
                    Nonce::from_slice(&self.nonce),
                    AeadPayload { msg: &self.bytes[start..end], aad: &self.bytes[..AAD_END] },
                )
                .map_err(|_| Error::DecryptFailed)?,
        );
        Payload::decode(&plaintext)
    }
}
