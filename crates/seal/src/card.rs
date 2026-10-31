//! Contact cards: a batch of signed one-time locks (requests) from one
//! computer, in one file. Send a card once; whoever opens it can seal a key to
//! you with each lock, and each lock opens once, exactly like a request.

use crate::identity::{Identity, PublicIdentity};
use crate::keyring::PendingRequest;
use crate::request::{Request, create_request};
use crate::wire::{self, Reader};
use crate::{Error, Kind};

/// Most locks one card may carry.
pub const MAX_CARD_LOCKS: usize = 32;

/// A verified card: every lock is a valid request from the same computer
/// under the same name. Holds no secret; safe to send anywhere.
#[derive(Clone, Debug)]
pub struct Card {
    bytes: Vec<u8>,
    locks: Vec<Request>,
}

/// Make a card with `count` locks. Returns the card to send and the pending
/// entries (the one-time private keys) to keep in the [`Keyring`].
///
/// [`Keyring`]: crate::Keyring
pub fn create_card(identity: &Identity, name: &str, now: u64, ttl: u64, count: usize) -> Result<(Card, Vec<PendingRequest>), Error> {
    if count == 0 || count > MAX_CARD_LOCKS {
        return Err(Error::InvalidField);
    }
    let mut bytes = Vec::new();
    wire::write_header(&mut bytes, Kind::Card);
    bytes.push(count as u8);
    let mut pending = Vec::with_capacity(count);
    for _ in 0..count {
        let (request, entry) = create_request(identity, name, now, ttl)?;
        bytes.extend_from_slice(&(request.to_bytes().len() as u16).to_be_bytes());
        bytes.extend_from_slice(request.to_bytes());
        pending.push(entry);
    }
    Ok((Card::parse(&bytes)?, pending))
}

impl Card {
    /// Parse a card and verify every lock and that they share one sender.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if wire::header(bytes)? != Kind::Card {
            return Err(Error::WrongKind);
        }
        let mut r = Reader::new(bytes, 5);
        let count = r.u8()? as usize;
        if count == 0 || count > MAX_CARD_LOCKS {
            return Err(Error::Malformed);
        }
        let mut locks: Vec<Request> = Vec::with_capacity(count);
        for _ in 0..count {
            let len = r.u16()? as usize;
            let lock = Request::parse(r.take(len)?)?;
            if let Some(first) = locks.first() {
                if lock.requester() != first.requester() || lock.name() != first.name() {
                    return Err(Error::Malformed);
                }
            }
            locks.push(lock);
        }
        r.finish()?;
        Ok(Self { bytes: bytes.to_vec(), locks })
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The locks, each a request a key can be sealed to once.
    pub fn locks(&self) -> &[Request] {
        &self.locks
    }

    /// The card holder's display name, as they typed it. Not verified by anyone.
    pub fn name(&self) -> &str {
        self.locks[0].name()
    }

    pub fn holder(&self) -> PublicIdentity {
        self.locks[0].requester()
    }
}
