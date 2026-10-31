//! Bundles: several sealed keys in one file, labelled with a team name. A key
//! shared with a team is sealed once per member, each to one of that member's
//! locks; each member's app opens the part sealed to it. A newcomer's welcome
//! is a bundle of every team key, all sealed to them.

use crate::sealed::Sealed;
use crate::wire::{self, Reader};
use crate::{Error, Kind, MAX_NAME_BYTES};

/// Most sealed keys one bundle may carry.
pub const MAX_BUNDLE_KEYS: usize = 200;

/// A verified bundle: every part is a sealed key with a valid sender signature.
/// The team name is a label, not verified by anyone. Holds only ciphertext.
#[derive(Clone, Debug)]
pub struct Bundle {
    bytes: Vec<u8>,
    team: String,
    parts: Vec<Sealed>,
}

impl Bundle {
    /// Put sealed keys in one bundle labelled `team`.
    pub fn new(team: &str, parts: &[Sealed]) -> Result<Self, Error> {
        if !wire::valid_text(team, MAX_NAME_BYTES, false) || parts.is_empty() || parts.len() > MAX_BUNDLE_KEYS {
            return Err(Error::InvalidField);
        }
        let mut bytes = Vec::new();
        wire::write_header(&mut bytes, Kind::Bundle);
        bytes.push(team.len() as u8);
        bytes.extend_from_slice(team.as_bytes());
        bytes.push(parts.len() as u8);
        for part in parts {
            let part = part.to_bytes();
            bytes.extend_from_slice(&u16::try_from(part.len()).map_err(|_| Error::InvalidField)?.to_be_bytes());
            bytes.extend_from_slice(part);
        }
        Self::parse(&bytes)
    }

    /// Parse a bundle and verify every part's signature.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if wire::header(bytes)? != Kind::Bundle {
            return Err(Error::WrongKind);
        }
        let mut r = Reader::new(bytes, 5);
        let team_len = r.u8()? as usize;
        let team = r.text(team_len, MAX_NAME_BYTES, false)?;
        let count = r.u8()? as usize;
        if count == 0 || count > MAX_BUNDLE_KEYS {
            return Err(Error::Malformed);
        }
        let mut parts = Vec::with_capacity(count);
        for _ in 0..count {
            let len = r.u16()? as usize;
            parts.push(Sealed::parse(r.take(len)?)?);
        }
        r.finish()?;
        Ok(Self { bytes: bytes.to_vec(), team, parts })
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn team(&self) -> &str {
        &self.team
    }

    /// The sealed keys; a recipient can open only those sealed to its locks.
    pub fn parts(&self) -> &[Sealed] {
        &self.parts
    }
}
