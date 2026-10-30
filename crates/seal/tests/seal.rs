//! Behaviour: a sealed key opens only with its request's one-time key,
//! and never once that key is destroyed or the request has run out.
//! Synthetic secrets only.

use keyhandoff_seal::{
    DEFAULT_REQUEST_TTL, Error, Identity, Keyring, Object, Payload, Request, Sealed, Secret, create_request, parse_object,
    seal,
};

const NOW: u64 = 1_800_000_000;
const SECRET: &[u8] = b"sk-test-NOT-A-REAL-KEY-0123456789";

fn payload() -> Payload {
    Payload { label: "Dialpad".into(), note: "for the call bot".into(), secret: Secret::new(SECRET.to_vec()) }
}

/// Ann asks; Ben seals. Returns Ann's keyring, the request and the sealed key.
fn handoff() -> (Keyring, Request, Sealed, Identity) {
    let ann = Identity::generate();
    let ben = Identity::generate();
    let (request, pending) = create_request(&ann, "Ann", NOW, DEFAULT_REQUEST_TTL).unwrap();
    let mut keyring = Keyring::new();
    keyring.insert(pending);
    // The request travels as bytes; Ben parses and verifies it.
    let received = Request::parse(request.to_bytes()).unwrap();
    let sealed = seal(&ben, &received, &payload(), NOW + 60).unwrap();
    (keyring, request, sealed, ben)
}

#[test]
fn round_trip() {
    let (mut keyring, _, sealed, ben) = handoff();
    let travelled = Sealed::parse(sealed.to_bytes()).unwrap();
    let opened = keyring.open(&travelled, NOW + 120).unwrap();
    assert_eq!(opened.payload.secret.expose(), SECRET);
    assert_eq!(opened.payload.label, "Dialpad");
    assert_eq!(opened.payload.note, "for the call bot");
    assert_eq!(opened.sender, ben.public());
    assert!(keyring.is_empty(), "the one-time key is destroyed on open");
}

#[test]
fn wrong_recipient() {
    let (_, _, sealed, _) = handoff();
    // Someone else's computer, with its own pending request.
    let other = Identity::generate();
    let (_, other_pending) = create_request(&other, "Mallory", NOW, DEFAULT_REQUEST_TTL).unwrap();
    let mut other_ring = Keyring::new();
    other_ring.insert(other_pending);
    assert_eq!(other_ring.open(&sealed, NOW + 120).unwrap_err(), Error::UnknownRequest);
    assert_eq!(other_ring.len(), 1, "a failed open leaves other keys alone");

    // Even given the right request id, a different one-time key cannot decrypt.
    let bytes = Keyring::from_bytes(&other_ring.to_bytes()).unwrap().to_bytes();
    let mut forged = bytes.to_vec();
    forged[8..24].copy_from_slice(&sealed.request_id());
    let mut forged_ring = Keyring::from_bytes(&forged).unwrap();
    assert_eq!(forged_ring.open(&sealed, NOW + 120).unwrap_err(), Error::DecryptFailed);
    assert_eq!(forged_ring.len(), 1, "a decrypt failure does not destroy the key");
}

#[test]
fn opened_after_destroy() {
    // Accepting destroys the key: a second copy of the same file never opens.
    let (mut keyring, _, sealed, _) = handoff();
    let copy = sealed.clone();
    keyring.open(&sealed, NOW + 120).unwrap();
    assert_eq!(keyring.open(&copy, NOW + 130).unwrap_err(), Error::UnknownRequest);

    // Withdrawing a request destroys the key before any open.
    let (mut keyring, request, sealed, _) = handoff();
    assert!(keyring.destroy(request.id()));
    assert_eq!(keyring.open(&sealed, NOW + 120).unwrap_err(), Error::UnknownRequest);

    // Destroyed keys stay destroyed across save and load.
    let (mut keyring, _, sealed, _) = handoff();
    keyring.open(&sealed, NOW + 120).unwrap();
    let mut reloaded = Keyring::from_bytes(&keyring.to_bytes()).unwrap();
    assert_eq!(reloaded.open(&sealed, NOW + 130).unwrap_err(), Error::UnknownRequest);
}

#[test]
fn expired_request() {
    let ann = Identity::generate();
    let ben = Identity::generate();
    let ttl = 3600;
    let (request, pending) = create_request(&ann, "Ann", NOW, ttl).unwrap();

    // The sender refuses to seal to an expired request.
    assert_eq!(seal(&ben, &request, &payload(), NOW + ttl).unwrap_err(), Error::Expired);

    // Sealed in time but opened late: refused, and the key is destroyed.
    let sealed = seal(&ben, &request, &payload(), NOW + ttl - 1).unwrap();
    let mut keyring = Keyring::new();
    keyring.insert(pending);
    assert_eq!(keyring.open(&sealed, NOW + ttl).unwrap_err(), Error::Expired);
    assert!(keyring.is_empty());
    assert_eq!(keyring.open(&sealed, NOW + ttl - 1).unwrap_err(), Error::UnknownRequest);
}

#[test]
fn purge_expired_removes_only_old_entries() {
    let me = Identity::generate();
    let mut keyring = Keyring::new();
    let (_, short) = create_request(&me, "Me", NOW, 60).unwrap();
    let (_, long) = create_request(&me, "Me", NOW, DEFAULT_REQUEST_TTL).unwrap();
    keyring.insert(short);
    keyring.insert(long);
    assert_eq!(keyring.purge_expired(NOW + 60), 1);
    assert_eq!(keyring.len(), 1);
}

#[test]
fn keyring_survives_save_and_load() {
    let (keyring, _, sealed, _) = handoff();
    let mut reloaded = Keyring::from_bytes(&keyring.to_bytes()).unwrap();
    assert_eq!(reloaded.open(&sealed, NOW + 120).unwrap().payload.secret.expose(), SECRET);
    assert_eq!(Keyring::from_bytes(b"HOFK\0\0\0\x01short").unwrap_err(), Error::Malformed);
}

#[test]
fn identity_survives_save_and_load() {
    let me = Identity::generate();
    let again = Identity::from_bytes(&me.to_bytes());
    assert_eq!(me.public(), again.public());
    assert_eq!(me.public().fingerprint().len(), 16);
}

#[test]
fn tampering_is_detected() {
    let (mut keyring, request, sealed, _) = handoff();

    // Any changed byte in a sealed key breaks the sender's signature.
    for i in 0..sealed.to_bytes().len() {
        let mut bytes = sealed.to_bytes().to_vec();
        bytes[i] ^= 0x01;
        assert!(Sealed::parse(&bytes).is_err(), "byte {i} changed but parse succeeded");
    }
    // Any changed byte in a request, likewise (a swapped lock is caught).
    for i in 0..request.to_bytes().len() {
        let mut bytes = request.to_bytes().to_vec();
        bytes[i] ^= 0x01;
        assert!(Request::parse(&bytes).is_err(), "byte {i} changed but parse succeeded");
    }
    // Rejected tampered copies did not touch the one-time key.
    assert_eq!(keyring.len(), 1);
    assert!(keyring.open(&sealed, NOW + 120).is_ok());
}

#[test]
fn kinds_and_versions() {
    let (_, request, sealed, _) = handoff();
    assert!(matches!(parse_object(request.to_bytes()), Ok(Object::Request(_))));
    assert!(matches!(parse_object(sealed.to_bytes()), Ok(Object::Sealed(_))));
    assert_eq!(Request::parse(sealed.to_bytes()).unwrap_err(), Error::WrongKind);
    assert_eq!(Sealed::parse(request.to_bytes()).unwrap_err(), Error::WrongKind);
    let mut newer = request.to_bytes().to_vec();
    newer[3] = 1;
    assert_eq!(parse_object(&newer).unwrap_err(), Error::UnsupportedVersion);
    assert_eq!(parse_object(b"PNG").unwrap_err(), Error::Malformed);
}

#[test]
fn field_limits() {
    let me = Identity::generate();
    assert_eq!(create_request(&me, "", NOW, 60).unwrap_err(), Error::InvalidField);
    assert_eq!(create_request(&me, &"x".repeat(65), NOW, 60).unwrap_err(), Error::InvalidField);
    assert_eq!(create_request(&me, "Spen\ncer", NOW, 60).unwrap_err(), Error::InvalidField);
    let (request, _) = create_request(&me, "Zoë", NOW, 60).unwrap();
    assert_eq!(request.name(), "Zoë");

    let empty = Payload { label: "X".into(), note: String::new(), secret: Secret::new(vec![]) };
    assert_eq!(seal(&me, &request, &empty, NOW).unwrap_err(), Error::InvalidField);
    let huge = Payload { label: "X".into(), note: String::new(), secret: Secret::new(vec![b'a'; 4097]) };
    assert_eq!(seal(&me, &request, &huge, NOW).unwrap_err(), Error::InvalidField);
    let max = Payload { label: "X".into(), note: String::new(), secret: Secret::new(vec![b'a'; 4096]) };
    assert!(seal(&me, &request, &max, NOW).is_ok());
}

#[test]
fn debug_never_prints_the_secret() {
    let p = payload();
    let shown = format!("{p:?}");
    assert!(!shown.contains("NOT-A-REAL-KEY"), "{shown}");
}
