//! Behaviour: a contact card carries one-time locks from one computer; each
//! lock seals one key that opens once. Synthetic secrets only.

use keyhandoff_seal::{
    armor, create_card, parse_object, seal, Card, Error, Identity, Keyring, Object, Payload, Secret, DEFAULT_CARD_TTL,
};

const NOW: u64 = 1_800_000_000;

fn payload(secret: &[u8]) -> Payload {
    Payload { label: "Dialpad".into(), note: "Ben".into(), secret: Secret::new(secret.to_vec()) }
}

#[test]
fn every_lock_seals_one_key_that_opens_once() {
    let ann = Identity::generate();
    let ben = Identity::generate();
    let (card, pending) = create_card(&ann, "Ann", NOW, DEFAULT_CARD_TTL, 3).unwrap();
    let mut keyring = Keyring::new();
    pending.into_iter().for_each(|p| keyring.insert(p));

    // The card travels as armored text; Ben reads it back.
    let Object::Card(received) = parse_object(&armor::decode(&armor::card_text(&card)).unwrap()).unwrap() else { panic!("not a card") };
    assert_eq!((received.name(), received.holder(), received.locks().len()), ("Ann", ann.public(), 3));

    for (i, lock) in received.locks().iter().enumerate() {
        let secret = format!("sk-test-card-{i}");
        let sealed = seal(&ben, lock, &payload(secret.as_bytes()), NOW + 60).unwrap();
        assert_eq!(keyring.open(&sealed, NOW + 120).unwrap().payload.secret.expose(), secret.as_bytes());
        assert_eq!(keyring.open(&sealed, NOW + 130).unwrap_err(), Error::UnknownRequest, "a lock opens once");
    }
    assert!(keyring.is_empty());
}

#[test]
fn locks_from_two_computers_or_changed_bytes_are_refused() {
    let ann = Identity::generate();
    let mallory = Identity::generate();
    let (card, _) = create_card(&ann, "Ann", NOW, DEFAULT_CARD_TTL, 2).unwrap();
    let (other, _) = create_card(&mallory, "Ann", NOW, DEFAULT_CARD_TTL, 1).unwrap();

    // Splice Mallory's lock into Ann's card: count 3, Ann's two locks, then Mallory's.
    let mut spliced = card.to_bytes().to_vec();
    spliced[5] = 3;
    spliced.extend_from_slice(&other.to_bytes()[6..]);
    assert_eq!(Card::parse(&spliced).unwrap_err(), Error::Malformed, "a lock from another computer");

    let mut changed = card.to_bytes().to_vec();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    assert!(Card::parse(&changed).is_err(), "a changed byte breaks a lock's signature");
    assert_eq!(create_card(&ann, "Ann", NOW, DEFAULT_CARD_TTL, 0).unwrap_err(), Error::InvalidField);
}
