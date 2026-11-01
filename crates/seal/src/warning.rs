//! A warning a team member writes into the team folder when one of their
//! programs tried to send a key's placeholder somewhere it doesn't belong:
//! where it tried to send it and which program, signed by that member's
//! computer. Teammates' KeyHandoffs refuse that destination for their own
//! programs. It holds no secret, and isn't encrypted.

use crate::identity::{Identity, PublicIdentity};
use crate::wire::{self, Reader};
use crate::{Error, Kind};

const WARNING_DOMAIN: &[u8] = b"handoff:team-warning:v0";
/// A host name or address.
pub const MAX_DESTINATION_BYTES: usize = 253;
/// The program, script or package that tried.
pub const MAX_CULPRIT_BYTES: usize = 128;

/// A verified warning.
#[derive(Clone, Debug)]
pub struct Warning {
    bytes: Vec<u8>,
    team_id: [u8; 16],
    reporter: PublicIdentity,
    at: u64,
    destination: String,
    culprit: String,
}

/// Sign a warning for the team `team_id`.
pub fn warn(reporter: &Identity, team_id: [u8; 16], destination: &str, culprit: &str, now: u64) -> Result<Warning, Error> {
    if !wire::valid_text(destination, MAX_DESTINATION_BYTES, false) || !wire::valid_text(culprit, MAX_CULPRIT_BYTES, true) {
        return Err(Error::InvalidField);
    }
    let mut bytes = Vec::new();
    wire::write_header(&mut bytes, Kind::Warning);
    bytes.extend_from_slice(&team_id);
    bytes.extend_from_slice(reporter.public().as_bytes());
    bytes.extend_from_slice(&now.to_be_bytes());
    for text in [destination, culprit] {
        bytes.push(text.len() as u8);
        bytes.extend_from_slice(text.as_bytes());
    }
    let signature = reporter.sign(WARNING_DOMAIN, &bytes);
    bytes.extend_from_slice(&signature);
    Warning::parse(&bytes)
}

impl Warning {
    /// Parse a warning and verify the reporter's signature. Whether the
    /// reporter is on the team is for the caller to check.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if wire::header(bytes)? != Kind::Warning {
            return Err(Error::WrongKind);
        }
        let mut r = Reader::new(bytes, 5);
        let team_id = r.array()?;
        let reporter = PublicIdentity::from_bytes(r.array()?)?;
        let at = r.u64()?;
        let len = r.u8()? as usize;
        let destination = r.text(len, MAX_DESTINATION_BYTES, false)?;
        let len = r.u8()? as usize;
        let culprit = r.text(len, MAX_CULPRIT_BYTES, true)?;
        let signed_len = r.pos();
        let signature = r.array()?;
        r.finish()?;
        reporter.verify(WARNING_DOMAIN, &bytes[..signed_len], &signature)?;
        Ok(Self { bytes: bytes.to_vec(), team_id, reporter, at, destination, culprit })
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn team_id(&self) -> [u8; 16] {
        self.team_id
    }

    /// The computer that saw it.
    pub fn reporter(&self) -> PublicIdentity {
        self.reporter
    }

    /// When, in Unix seconds, by the reporter's clock.
    pub fn at(&self) -> u64 {
        self.at
    }

    /// Where the placeholder was going: a host name or an address.
    pub fn destination(&self) -> &str {
        &self.destination
    }

    /// The program or script that tried, as the reporter's KeyHandoff named it.
    pub fn culprit(&self) -> &str {
        &self.culprit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_warning_verifies_and_a_changed_byte_does_not() {
        let ann = Identity::generate();
        let w = warn(&ann, [7; 16], "evil.test", "python3 pay.py", 1_000).unwrap();
        let back = Warning::parse(w.to_bytes()).unwrap();
        assert_eq!((back.destination(), back.culprit(), back.team_id(), back.at()), ("evil.test", "python3 pay.py", [7; 16], 1_000));
        assert_eq!(back.reporter(), ann.public());
        let text = crate::armor::encode(w.to_bytes(), "A warning.").unwrap();
        assert!(matches!(crate::parse_object(&crate::armor::decode(&text).unwrap()), Ok(crate::Object::Warning(_))), "survives the folder as text");
        let mut bytes = w.to_bytes().to_vec();
        let at = bytes.len() - 70;
        bytes[at] ^= 1;
        assert!(Warning::parse(&bytes).is_err());
        assert!(warn(&ann, [7; 16], "", "x", 0).is_err());
    }
}
