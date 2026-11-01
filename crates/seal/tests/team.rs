//! Behaviour: a team kept in a shared folder. The keeper invites, people join
//! with a team lock, and each team state opens only for the members it was
//! sealed for. Synthetic secrets only.

use keyhandoff_seal::{
    armor, create_invite, join, parse_object, reinvite, seal_team, Error, Identity, Invite, Membership, Object, Payload, Secret,
    TeamLock, TeamState,
};

const NOW: u64 = 1_800_000_000;
const WEEK: u64 = 7 * 24 * 60 * 60;

fn key(label: &str, secret: &str) -> Payload {
    Payload { label: label.into(), note: String::new(), secret: Secret::new(secret.as_bytes().to_vec()) }
}

fn secrets(state: &TeamState, lock: &TeamLock) -> Vec<(String, Vec<u8>)> {
    state.open(lock).unwrap().into_iter().map(|p| (p.label, p.secret.expose().to_vec())).collect()
}

/// Ben keeps Platform; Ann and Priya join through the folder.
struct Team {
    ben: Identity,
    invite: Invite,
    ann: (Membership, TeamLock),
    priya: (Membership, TeamLock),
}

fn platform() -> Team {
    let ben = Identity::generate();
    let invite = create_invite(&ben, "Platform", NOW, WEEK).unwrap();
    // The invite and the replies travel as files in the folder; read them back as the other side would.
    let Object::Invite(received) = parse_object(invite.to_bytes()).unwrap() else { panic!("not an invite") };
    let ann = join(&Identity::generate(), "Ann", &received, NOW + 60).unwrap();
    let priya = join(&Identity::generate(), "Priya", &received, NOW + 60).unwrap();
    Team { ben, invite, ann, priya }
}

#[test]
fn members_open_the_team_keys_and_no_one_else_does() {
    let t = platform();
    let members = [t.ann.0.clone(), t.priya.0.clone()];
    let state = seal_team(&t.ben, t.invite.team_id(), "Platform", 1, &members, &[key("Stripe", "sk_test_1"), key("OpenAI", "sk-test-2")], NOW + 120).unwrap();

    // The state goes into the folder as text; each member reads it back.
    let Object::TeamState(read) = parse_object(&armor::decode(&armor::encode(state.to_bytes(), "Platform team").unwrap()).unwrap()).unwrap()
    else {
        panic!("not a team state")
    };
    assert_eq!((read.keeper(), read.version(), read.team()), (t.ben.public(), 1, "Platform"));
    let expected = vec![("Stripe".to_string(), b"sk_test_1".to_vec()), ("OpenAI".to_string(), b"sk-test-2".to_vec())];
    assert_eq!(secrets(&read, &t.ann.1), expected);
    // A lock kept in the vault and loaded back still opens it.
    assert_eq!(secrets(&read, &TeamLock::from_bytes(&t.priya.1.to_bytes())), expected);

    let outsider = join(&Identity::generate(), "Mallory", &t.invite, NOW + 60).unwrap().1;
    assert_eq!(read.open(&outsider).unwrap_err(), Error::UnknownRequest, "joining isn't enough; the keeper adds people");
}

#[test]
fn rotating_reaches_everyone_and_a_removed_member_gets_nothing_new() {
    let t = platform();
    let both = [t.ann.0.clone(), t.priya.0.clone()];
    let v1 = seal_team(&t.ben, t.invite.team_id(), "Platform", 1, &both, &[key("Stripe", "sk_test_old")], NOW + 120).unwrap();
    let v2 = seal_team(&t.ben, t.invite.team_id(), "Platform", 2, &both, &[key("Stripe", "sk_test_new")], NOW + 180).unwrap();
    assert_eq!(secrets(&v2, &t.ann.1), vec![("Stripe".to_string(), b"sk_test_new".to_vec())]);
    assert_eq!(secrets(&v2, &t.priya.1), vec![("Stripe".to_string(), b"sk_test_new".to_vec())]);

    // Priya leaves: version 3 is sealed for Ann only.
    let v3 = seal_team(&t.ben, t.invite.team_id(), "Platform", 3, &[t.ann.0.clone()], &[key("Stripe", "sk_test_newer")], NOW + 240).unwrap();
    assert_eq!(secrets(&v3, &t.ann.1), vec![("Stripe".to_string(), b"sk_test_newer".to_vec())]);
    assert_eq!(v3.open(&t.priya.1).unwrap_err(), Error::UnknownRequest);
    assert_eq!(v3.members().iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Ann"]);
    // What she was already given stays readable to her; that's why leaving prompts a rotation.
    assert!(v1.open(&t.priya.1).is_ok());
}

#[test]
fn people_for_another_team_or_keeper_are_refused() {
    let t = platform();
    let other_invite = create_invite(&t.ben, "Billing", NOW, WEEK).unwrap();
    let elsewhere = join(&Identity::generate(), "Sam", &other_invite, NOW).unwrap().0;
    let refused = seal_team(&t.ben, t.invite.team_id(), "Platform", 1, &[t.ann.0.clone(), elsewhere], &[], NOW);
    assert_eq!(refused.unwrap_err(), Error::WrongKind);

    // Someone else can't write a state for Ben's team with Ben's members.
    let mallory = Identity::generate();
    let forged = seal_team(&mallory, t.invite.team_id(), "Platform", 9, &[t.ann.0.clone()], &[], NOW);
    assert_eq!(forged.unwrap_err(), Error::WrongKind);

    // The same person or lock twice is refused.
    let twice = seal_team(&t.ben, t.invite.team_id(), "Platform", 1, &[t.ann.0.clone(), t.ann.0.clone()], &[], NOW);
    assert_eq!(twice.unwrap_err(), Error::InvalidField);
}

#[test]
fn changed_files_and_old_invites_are_refused() {
    let t = platform();
    let state = seal_team(&t.ben, t.invite.team_id(), "Platform", 1, &[t.ann.0.clone()], &[key("Stripe", "sk_test_1")], NOW).unwrap();
    for at in [6, 60, state.to_bytes().len() - 100] {
        let mut bytes = state.to_bytes().to_vec();
        bytes[at] ^= 1;
        assert!(TeamState::parse(&bytes).is_err(), "a changed byte at {at} is refused");
    }
    let mut reply = t.ann.0.to_bytes().to_vec();
    let last = reply.len() - 1;
    reply[last] ^= 1;
    assert_eq!(Membership::parse(&reply).unwrap_err(), Error::BadSignature);

    assert_eq!(join(&Identity::generate(), "Late", &t.invite, NOW + WEEK).unwrap_err(), Error::Expired);
    // A fresh invite to the same team lets them in, and their reply counts for it.
    let again = reinvite(&t.ben, t.invite.team_id(), "Platform", NOW + WEEK, WEEK).unwrap();
    let late = join(&Identity::generate(), "Late", &again, NOW + WEEK + 1).unwrap();
    let state = seal_team(&t.ben, t.invite.team_id(), "Platform", 2, &[t.ann.0.clone(), late.0], &[key("Stripe", "sk_test_1")], NOW + WEEK + 2).unwrap();
    assert_eq!(secrets(&state, &late.1), vec![("Stripe".to_string(), b"sk_test_1".to_vec())]);
}
