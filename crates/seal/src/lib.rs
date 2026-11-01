//! Handoff sealing core: device identity, key requests with one-time keys,
//! contact cards that carry a batch of them, sealing a secret to a request,
//! bundles of sealed keys for a team, teams kept in a shared folder,
//! opening it exactly once, and the armored text that carries them through
//! chat and email.
//!
//! Design: `docs/sealing-format.md`. This crate has no UI, OS
//! keychain or network code so it can be reviewed on its own.
//! Callers pass the current time in Unix seconds; the crate never reads a clock.

pub mod armor;
mod bundle;
mod card;
mod identity;
mod keyring;
mod request;
mod sealed;
mod team;
mod wire;

use std::fmt;

pub use bundle::{Bundle, MAX_BUNDLE_KEYS};
pub use card::{Card, MAX_CARD_LOCKS, create_card};
pub use identity::{Identity, PublicIdentity};
pub use keyring::{Keyring, Opened, PendingRequest};
pub use request::{Request, create_request};
pub use sealed::{Payload, Sealed, Secret, seal};
pub use team::{Invite, MAX_TEAM_MEMBERS, Member, Membership, TeamLock, TeamState, create_invite, join, reinvite, rejoin, seal_team};

/// Format version written into every object (`HOF` + version + kind).
pub const FORMAT_VERSION: u8 = 0;
/// Default request lifetime: 7 days.
pub const DEFAULT_REQUEST_TTL: u64 = 7 * 24 * 60 * 60;
/// Default contact card lifetime: 180 days.
pub const DEFAULT_CARD_TTL: u64 = 180 * 24 * 60 * 60;
pub const MAX_NAME_BYTES: usize = 64;
pub const MAX_LABEL_BYTES: usize = 64;
pub const MAX_NOTE_BYTES: usize = 200;
pub const MAX_SECRET_BYTES: usize = 4096;

const MAGIC: &[u8; 3] = b"HOF";

/// Which kind of `.handoff` object some bytes hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Request = 1,
    Sealed = 2,
    Card = 3,
    Bundle = 4,
    Invite = 5,
    Membership = 6,
    TeamState = 7,
}

impl Kind {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(Kind::Request),
            2 => Some(Kind::Sealed),
            3 => Some(Kind::Card),
            4 => Some(Kind::Bundle),
            5 => Some(Kind::Invite),
            6 => Some(Kind::Membership),
            7 => Some(Kind::TeamState),
            _ => None,
        }
    }
}

/// A parsed `.handoff` object of any kind.
#[derive(Debug)]
pub enum Object {
    Request(Request),
    Sealed(Sealed),
    Card(Card),
    Bundle(Bundle),
    Invite(Invite),
    Membership(Membership),
    TeamState(TeamState),
}

/// Parse and verify any `.handoff` object from its binary form.
pub fn parse_object(bytes: &[u8]) -> Result<Object, Error> {
    match wire::header(bytes)? {
        Kind::Request => Request::parse(bytes).map(Object::Request),
        Kind::Sealed => Sealed::parse(bytes).map(Object::Sealed),
        Kind::Card => Card::parse(bytes).map(Object::Card),
        Kind::Bundle => Bundle::parse(bytes).map(Object::Bundle),
        Kind::Invite => Invite::parse(bytes).map(Object::Invite),
        Kind::Membership => Membership::parse(bytes).map(Object::Membership),
        Kind::TeamState => TeamState::parse(bytes).map(Object::TeamState),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Not a Handoff object, or truncated or malformed.
    Malformed,
    /// Made by a newer KeyHandoff.
    UnsupportedVersion,
    /// A request where a sealed key was expected, or the other way round.
    WrongKind,
    /// The signature does not match: the object was changed or forged.
    BadSignature,
    /// The request ran out.
    Expired,
    /// No one-time key for this request: already opened, expired, or never made here.
    UnknownRequest,
    /// The one-time key does not open this object.
    DecryptFailed,
    /// A name, label, note or secret is empty, too long or has control characters.
    InvalidField,
    /// No randomness available from the operating system.
    Randomness,
    /// Armored text: no BEGIN HANDOFF marker found.
    NotHandoffText,
    /// Armored text: damaged beyond what the parity can repair.
    TooDamaged,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::Malformed => "This is not a KeyHandoff file, or it is incomplete.",
            Error::UnsupportedVersion => "This was made by a newer KeyHandoff. Update to open it.",
            Error::WrongKind => "This is a different kind of KeyHandoff file.",
            Error::BadSignature => "This file was changed after it was made. Ask them to send it again.",
            Error::Expired | Error::UnknownRequest => {
                "This key was already opened, or the request ran out. Ask again."
            }
            Error::DecryptFailed => "This key was sealed for a different request.",
            Error::InvalidField => "A name, label or note is empty, too long or has hidden characters.",
            Error::Randomness => "The computer could not provide secure randomness.",
            Error::NotHandoffText => "No KeyHandoff message was found in this text.",
            Error::TooDamaged => {
                "This message is too damaged to open. Ask them to send it again."
            }
        })
    }
}

impl std::error::Error for Error {}
