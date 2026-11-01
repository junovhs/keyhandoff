//! Teams kept in a shared folder. The keeper writes an invite; each person who
//! joins writes a membership with a long-lived team lock; the keeper writes a
//! versioned team state that carries the members and the team's keys,
//! encrypted once and wrapped for each member's lock. Removing someone means
//! the next version is not wrapped for them. Whatever syncs the folder carries
//! these files; they hold only ciphertext and public keys.

use chacha20poly1305::aead::{Aead, KeyInit, Payload as AeadPayload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

use crate::identity::{Identity, PublicIdentity};
use crate::sealed::Payload;
use crate::wire::{self, Reader};
use crate::{Error, Kind, MAX_BUNDLE_KEYS, MAX_NAME_BYTES};

const INVITE_DOMAIN: &[u8] = b"handoff:team-invite:v0";
const MEMBER_DOMAIN: &[u8] = b"handoff:team-member:v0";
const STATE_DOMAIN: &[u8] = b"handoff:team-state:v0";
const KDF_INFO: &[u8] = b"handoff:team-wrap:v0";
const TAG_BYTES: usize = 16;
/// A wrapped content key: 32 bytes and its tag.
const SLOT_BYTES: usize = 32 + TAG_BYTES;

/// Most people one team state may carry.
pub const MAX_TEAM_MEMBERS: usize = 64;

/// A member's private team lock: opens every state of one team that was
/// wrapped for it. Persist with [`TeamLock::to_bytes`] under the vault's
/// protection; zeroized on drop.
pub struct TeamLock {
    secret: StaticSecret,
}

impl TeamLock {
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self { secret: StaticSecret::from(*bytes) }
    }

    pub fn to_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.secret.to_bytes())
    }

    pub fn public(&self) -> [u8; 32] {
        PublicKey::from(&self.secret).to_bytes()
    }
}

impl std::fmt::Debug for TeamLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TeamLock")
    }
}

/// A verified invite: "join this team, kept by this computer". Holds no secret.
#[derive(Clone, Debug)]
pub struct Invite {
    bytes: Vec<u8>,
    team_id: [u8; 16],
    team: String,
    keeper: PublicIdentity,
    expires_at: u64,
}

/// Make an invite to a new team. The team id is random and names the team for
/// good; the name can change in later states.
pub fn create_invite(keeper: &Identity, team: &str, now: u64, ttl: u64) -> Result<Invite, Error> {
    if !wire::valid_text(team, MAX_NAME_BYTES, false) || ttl == 0 {
        return Err(Error::InvalidField);
    }
    let expires_at = now.checked_add(ttl).ok_or(Error::InvalidField)?;
    let mut team_id = [0u8; 16];
    OsRng.try_fill_bytes(&mut team_id).map_err(|_| Error::Randomness)?;
    invite_for(keeper, team_id, team, now, expires_at)
}

/// Another invite to an existing team, for people who join later.
pub fn reinvite(keeper: &Identity, team_id: [u8; 16], team: &str, now: u64, ttl: u64) -> Result<Invite, Error> {
    let expires_at = now.checked_add(ttl).ok_or(Error::InvalidField)?;
    invite_for(keeper, team_id, team, now, expires_at)
}

fn invite_for(keeper: &Identity, team_id: [u8; 16], team: &str, now: u64, expires_at: u64) -> Result<Invite, Error> {
    if !wire::valid_text(team, MAX_NAME_BYTES, false) || expires_at <= now {
        return Err(Error::InvalidField);
    }
    let mut bytes = Vec::new();
    wire::write_header(&mut bytes, Kind::Invite);
    bytes.extend_from_slice(&team_id);
    bytes.extend_from_slice(keeper.public().as_bytes());
    bytes.extend_from_slice(&now.to_be_bytes());
    bytes.extend_from_slice(&expires_at.to_be_bytes());
    push_text(&mut bytes, team);
    let signature = keeper.sign(INVITE_DOMAIN, &bytes);
    bytes.extend_from_slice(&signature);
    Invite::parse(&bytes)
}

impl Invite {
    /// Parse an invite and verify the keeper's signature.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if wire::header(bytes)? != Kind::Invite {
            return Err(Error::WrongKind);
        }
        let mut r = Reader::new(bytes, 5);
        let team_id = r.array()?;
        let keeper = PublicIdentity::from_bytes(r.array()?)?;
        let created_at = r.u64()?;
        let expires_at = r.u64()?;
        let len = r.u8()? as usize;
        let team = r.text(len, MAX_NAME_BYTES, false)?;
        let signed_len = r.pos();
        let signature = r.array()?;
        r.finish()?;
        if expires_at <= created_at {
            return Err(Error::Malformed);
        }
        keeper.verify(INVITE_DOMAIN, &bytes[..signed_len], &signature)?;
        Ok(Self { bytes: bytes.to_vec(), team_id, team, keeper, expires_at })
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn team_id(&self) -> [u8; 16] {
        self.team_id
    }

    /// The team's name as the keeper typed it.
    pub fn team(&self) -> &str {
        &self.team
    }

    /// The computer that keeps the team. Joining pins it: only states it signs
    /// are accepted for this team.
    pub fn keeper(&self) -> PublicIdentity {
        self.keeper
    }

    pub fn is_expired(&self, now: u64) -> bool {
        now >= self.expires_at
    }
}

/// A verified membership: "this computer asks to join this team, and here is
/// its team lock". Holds no secret. The keeper decides whether to add it.
#[derive(Clone, Debug)]
pub struct Membership {
    bytes: Vec<u8>,
    team_id: [u8; 16],
    keeper: PublicIdentity,
    member: PublicIdentity,
    lock: [u8; 32],
    name: String,
}

/// Answer an invite: make a team lock and the membership to put in the team
/// folder. Keep the lock; it opens this team's states from now on.
pub fn join(member: &Identity, name: &str, invite: &Invite, now: u64) -> Result<(Membership, TeamLock), Error> {
    if invite.is_expired(now) {
        return Err(Error::Expired);
    }
    if !wire::valid_text(name, MAX_NAME_BYTES, false) {
        return Err(Error::InvalidField);
    }
    let lock = TeamLock { secret: StaticSecret::random_from_rng(OsRng) };
    let mut bytes = Vec::new();
    wire::write_header(&mut bytes, Kind::Membership);
    bytes.extend_from_slice(&invite.team_id);
    bytes.extend_from_slice(invite.keeper.as_bytes());
    bytes.extend_from_slice(member.public().as_bytes());
    bytes.extend_from_slice(&lock.public());
    bytes.extend_from_slice(&now.to_be_bytes());
    push_text(&mut bytes, name);
    let signature = member.sign(MEMBER_DOMAIN, &bytes);
    bytes.extend_from_slice(&signature);
    Ok((Membership::parse(&bytes)?, lock))
}

impl Membership {
    /// Parse a membership and verify the joiner's signature.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if wire::header(bytes)? != Kind::Membership {
            return Err(Error::WrongKind);
        }
        let mut r = Reader::new(bytes, 5);
        let team_id = r.array()?;
        let keeper = PublicIdentity::from_bytes(r.array()?)?;
        let member = PublicIdentity::from_bytes(r.array()?)?;
        let lock = r.array()?;
        r.u64()?;
        let len = r.u8()? as usize;
        let name = r.text(len, MAX_NAME_BYTES, false)?;
        let signed_len = r.pos();
        let signature = r.array()?;
        r.finish()?;
        member.verify(MEMBER_DOMAIN, &bytes[..signed_len], &signature)?;
        Ok(Self { bytes: bytes.to_vec(), team_id, keeper, member, lock, name })
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn team_id(&self) -> [u8; 16] {
        self.team_id
    }

    pub fn keeper(&self) -> PublicIdentity {
        self.keeper
    }

    /// The computer asking to join. Show its fingerprint before adding it.
    pub fn member(&self) -> PublicIdentity {
        self.member
    }

    /// The joiner's display name, as they typed it. Not verified by anyone.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// One person in a team state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub identity: PublicIdentity,
    pub lock: [u8; 32],
    pub name: String,
}

impl From<&Membership> for Member {
    fn from(m: &Membership) -> Self {
        Self { identity: m.member, lock: m.lock, name: m.name.clone() }
    }
}

/// A verified team state: version `n` of a team, signed by its keeper. The
/// member list is readable by anyone with the file; the keys are not.
#[derive(Clone, Debug)]
pub struct TeamState {
    bytes: Vec<u8>,
    team_id: [u8; 16],
    keeper: PublicIdentity,
    version: u64,
    team: String,
    members: Vec<Member>,
    ephemeral: [u8; 32],
    slots_at: usize,
    nonce: [u8; 12],
    ciphertext_range: (usize, usize),
}

/// Write version `version` of a team: `keys` sealed for exactly `members`.
/// Every membership must answer this keeper's invite to this team.
pub fn seal_team(
    keeper: &Identity,
    team_id: [u8; 16],
    team: &str,
    version: u64,
    members: &[Membership],
    keys: &[Payload],
    now: u64,
) -> Result<TeamState, Error> {
    if !wire::valid_text(team, MAX_NAME_BYTES, false)
        || version == 0
        || members.is_empty()
        || members.len() > MAX_TEAM_MEMBERS
        || keys.len() > MAX_BUNDLE_KEYS
    {
        return Err(Error::InvalidField);
    }
    for (i, m) in members.iter().enumerate() {
        if m.team_id != team_id || m.keeper != keeper.public() {
            return Err(Error::WrongKind);
        }
        if members[..i].iter().any(|o| o.member == m.member || o.lock == m.lock) {
            return Err(Error::InvalidField);
        }
    }

    let mut plaintext = Zeroizing::new(vec![keys.len() as u8]);
    for key in keys {
        let encoded = key.encode()?;
        plaintext.extend_from_slice(&(encoded.len() as u16).to_be_bytes());
        plaintext.extend_from_slice(&encoded);
    }

    let mut bytes = Vec::new();
    wire::write_header(&mut bytes, Kind::TeamState);
    bytes.extend_from_slice(&team_id);
    bytes.extend_from_slice(keeper.public().as_bytes());
    bytes.extend_from_slice(&version.to_be_bytes());
    bytes.extend_from_slice(&now.to_be_bytes());
    push_text(&mut bytes, team);
    bytes.push(members.len() as u8);
    for m in members {
        bytes.extend_from_slice(m.member.as_bytes());
        bytes.extend_from_slice(&m.lock);
        push_text(&mut bytes, &m.name);
    }

    // One content key for the keys, wrapped once per member's lock.
    let mut content_key = Zeroizing::new([0u8; 32]);
    OsRng.try_fill_bytes(&mut content_key[..]).map_err(|_| Error::Randomness)?;
    let ephemeral = StaticSecret::random_from_rng(OsRng);
    let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
    bytes.extend_from_slice(&ephemeral_public);
    let header = bytes.clone();
    for m in members {
        let wrap = wrap_key(&ephemeral, &m.lock, &ephemeral_public, &m.lock, &header)?;
        let slot = ChaCha20Poly1305::new(Key::from_slice(&wrap[..]))
            .encrypt(Nonce::from_slice(&[0u8; 12]), AeadPayload { msg: &content_key[..], aad: &header })
            .map_err(|_| Error::Malformed)?;
        bytes.extend_from_slice(&slot);
    }

    let mut nonce = [0u8; 12];
    OsRng.try_fill_bytes(&mut nonce).map_err(|_| Error::Randomness)?;
    let aad_end = bytes.len();
    let ciphertext = ChaCha20Poly1305::new(Key::from_slice(&content_key[..]))
        .encrypt(Nonce::from_slice(&nonce), AeadPayload { msg: &plaintext, aad: &bytes[..aad_end] })
        .map_err(|_| Error::Malformed)?;
    bytes.extend_from_slice(&nonce);
    bytes.extend_from_slice(&u32::try_from(ciphertext.len()).map_err(|_| Error::InvalidField)?.to_be_bytes());
    bytes.extend_from_slice(&ciphertext);
    let signature = keeper.sign(STATE_DOMAIN, &bytes);
    bytes.extend_from_slice(&signature);
    TeamState::parse(&bytes)
}

/// The key that wraps the content key for one member: Diffie-Hellman between
/// the state's ephemeral key and the member's lock, from whichever side holds
/// a secret. Each state has a fresh ephemeral key, so every wrap key is used
/// once and a fixed nonce is safe. The salt is the state's public header, so a
/// slot can't be moved to another state.
fn wrap_key(ours: &StaticSecret, theirs: &[u8; 32], ephemeral: &[u8; 32], lock: &[u8; 32], header: &[u8]) -> Result<Zeroizing<[u8; 32]>, Error> {
    let shared = ours.diffie_hellman(&PublicKey::from(*theirs));
    if !shared.was_contributory() {
        return Err(Error::DecryptFailed);
    }
    let mut key = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(Some(header), shared.as_bytes())
        .expand_multi_info(&[KDF_INFO, ephemeral, lock], &mut key[..])
        .expect("32 bytes is a valid HKDF-SHA256 length");
    Ok(key)
}

impl TeamState {
    /// Parse a team state and verify the keeper's signature. Whether this
    /// keeper is the one you joined, and whether the version is newer than the
    /// last you saw, is for the app to check.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if wire::header(bytes)? != Kind::TeamState {
            return Err(Error::WrongKind);
        }
        let mut r = Reader::new(bytes, 5);
        let team_id = r.array()?;
        let keeper = PublicIdentity::from_bytes(r.array()?)?;
        let version = r.u64()?;
        r.u64()?;
        let len = r.u8()? as usize;
        let team = r.text(len, MAX_NAME_BYTES, false)?;
        let count = r.u8()? as usize;
        if count == 0 || count > MAX_TEAM_MEMBERS || version == 0 {
            return Err(Error::Malformed);
        }
        let mut members: Vec<Member> = Vec::with_capacity(count);
        for _ in 0..count {
            let identity = PublicIdentity::from_bytes(r.array()?)?;
            let lock = r.array()?;
            let len = r.u8()? as usize;
            let name = r.text(len, MAX_NAME_BYTES, false)?;
            members.push(Member { identity, lock, name });
        }
        let ephemeral = r.array()?;
        let slots_at = r.pos();
        r.take(count * SLOT_BYTES)?;
        let nonce = r.array()?;
        let ciphertext_len = r.u32()? as usize;
        let start = r.pos();
        if ciphertext_len < TAG_BYTES {
            return Err(Error::Malformed);
        }
        r.take(ciphertext_len)?;
        let signed_len = r.pos();
        let signature = r.array()?;
        r.finish()?;
        keeper.verify(STATE_DOMAIN, &bytes[..signed_len], &signature)?;
        Ok(Self { bytes: bytes.to_vec(), team_id, keeper, version, team, members, ephemeral, slots_at, nonce, ciphertext_range: (start, signed_len) })
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn team_id(&self) -> [u8; 16] {
        self.team_id
    }

    pub fn keeper(&self) -> PublicIdentity {
        self.keeper
    }

    /// Higher is newer. Apps refuse a version at or below the last one opened.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn team(&self) -> &str {
        &self.team
    }

    pub fn members(&self) -> &[Member] {
        &self.members
    }

    /// Open the keys with a member's team lock. A lock this version isn't
    /// wrapped for, like a removed member's, gets `UnknownRequest`.
    pub fn open(&self, lock: &TeamLock) -> Result<Vec<Payload>, Error> {
        let ours = lock.public();
        let index = self.members.iter().position(|m| m.lock == ours).ok_or(Error::UnknownRequest)?;
        let header = &self.bytes[..self.slots_at];
        let wrap = wrap_key(&lock.secret, &self.ephemeral, &self.ephemeral, &ours, header)?;
        let slot = &self.bytes[self.slots_at + index * SLOT_BYTES..][..SLOT_BYTES];
        let mut content_key: [u8; 32] = ChaCha20Poly1305::new(Key::from_slice(&wrap[..]))
            .decrypt(Nonce::from_slice(&[0u8; 12]), AeadPayload { msg: slot, aad: header })
            .map_err(|_| Error::DecryptFailed)?
            .try_into()
            .map_err(|_| Error::DecryptFailed)?;
        let (start, end) = self.ciphertext_range;
        let plaintext = ChaCha20Poly1305::new(Key::from_slice(&content_key))
            .decrypt(
                Nonce::from_slice(&self.nonce),
                AeadPayload { msg: &self.bytes[start..end], aad: &self.bytes[..start - 12 - 4] },
            )
            .map_err(|_| Error::DecryptFailed);
        content_key.zeroize();
        let plaintext = Zeroizing::new(plaintext?);
        let mut r = Reader::new(&plaintext, 0);
        let count = r.u8()? as usize;
        let mut keys = Vec::with_capacity(count);
        for _ in 0..count {
            let len = r.u16()? as usize;
            keys.push(Payload::decode(r.take(len)?)?);
        }
        r.finish()?;
        Ok(keys)
    }
}

fn push_text(out: &mut Vec<u8>, text: &str) {
    out.push(text.len() as u8);
    out.extend_from_slice(text.as_bytes());
}
