//! Long-term device identity: an Ed25519 key pair that signs requests and
//! sealed keys, so others can notice when a known computer's key changes.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand_core::OsRng;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::Error;

/// This computer's private identity. Persist it with [`Identity::to_bytes`]
/// under OS protection (DPAPI on Windows); the key is zeroized on drop.
pub struct Identity {
    signing: SigningKey,
}

impl Identity {
    pub fn generate() -> Self {
        Self { signing: SigningKey::generate(&mut OsRng) }
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self { signing: SigningKey::from_bytes(bytes) }
    }

    pub fn to_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.signing.to_bytes())
    }

    pub fn public(&self) -> PublicIdentity {
        PublicIdentity(self.signing.verifying_key().to_bytes())
    }

    pub(crate) fn sign(&self, domain: &[u8], body: &[u8]) -> [u8; 64] {
        self.signing.sign(&[domain, body].concat()).to_bytes()
    }
}

/// A computer's public identity, safe to show and store.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PublicIdentity(pub(crate) [u8; 32]);

impl PublicIdentity {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, Error> {
        VerifyingKey::from_bytes(&bytes).map_err(|_| Error::Malformed)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Short stable id for logs and continuity records (first 8 bytes of SHA-256, hex).
    pub fn fingerprint(&self) -> String {
        Sha256::digest(self.0)[..8].iter().map(|b| format!("{b:02x}")).collect()
    }

    pub(crate) fn verify(&self, domain: &[u8], body: &[u8], signature: &[u8; 64]) -> Result<(), Error> {
        let key = VerifyingKey::from_bytes(&self.0).map_err(|_| Error::Malformed)?;
        key.verify_strict(&[domain, body].concat(), &Signature::from_bytes(signature))
            .map_err(|_| Error::BadSignature)
    }
}
