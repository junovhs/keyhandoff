//! Behaviour: a sealed key still opens after typical transit damage,
//! and text damaged beyond the parity budget is refused, never misread.
//! Synthetic secrets only.

use keyhandoff_seal::armor::{self, decode, encode};
use keyhandoff_seal::{DEFAULT_REQUEST_TTL, Error, Identity, Keyring, Payload, Request, Sealed, Secret, create_request, seal};

const NOW: u64 = 1_800_000_000;
const SECRET: &[u8] = b"sk-test-NOT-A-REAL-KEY-abcdefghijklmnopqrstuvwxyz-0123456789";

fn handoff() -> (Keyring, Request, Sealed) {
    let ann = Identity::generate();
    let ben = Identity::generate();
    let (request, pending) = create_request(&ann, "Ann", NOW, DEFAULT_REQUEST_TTL).unwrap();
    let mut keyring = Keyring::new();
    keyring.insert(pending);
    let payload = Payload { label: "Anthropic".into(), note: String::new(), secret: Secret::new(SECRET.to_vec()) };
    let sealed = seal(&ben, &request, &payload, NOW + 60).unwrap();
    (keyring, request, sealed)
}

/// The armored body lines (between the markers).
fn body_lines(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines.iter().position(|l| l.contains("BEGIN")).unwrap();
    let end = lines.iter().position(|l| l.contains("END")).unwrap();
    lines[begin + 1..end].iter().map(|s| s.to_string()).collect()
}

fn rebuild(text: &str, body: &[String]) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let begin = lines.iter().position(|l| l.contains("BEGIN")).unwrap();
    let end = lines.iter().position(|l| l.contains("END")).unwrap();
    let mut out: Vec<String> = lines[..=begin].iter().map(|s| s.to_string()).collect();
    out.extend(body.iter().cloned());
    out.extend(lines[end..].iter().map(|s| s.to_string()));
    out.join("\n")
}

/// Replace the character at `index` among body characters (skipping spaces).
fn corrupt_char(text: &str, index: usize, with: char) -> String {
    let mut body = body_lines(text);
    let mut seen = 0;
    'outer: for line in body.iter_mut() {
        let mut chars: Vec<char> = line.chars().collect();
        for c in chars.iter_mut() {
            if *c == ' ' {
                continue;
            }
            if seen == index {
                *c = if *c == with { '7' } else { with };
                *line = chars.into_iter().collect();
                break 'outer;
            }
            seen += 1;
        }
    }
    rebuild(text, &body)
}

#[test]
fn round_trip_opens_the_key() {
    let (mut keyring, request, sealed) = handoff();
    let text = armor::sealed_text(&sealed, "Ann");
    assert!(text.starts_with("A key for Ann."));
    assert!(text.contains("-----BEGIN HANDOFF SEALED KEY-----"));
    assert!(!text.contains("NOT-A-REAL-KEY"));
    let travelled = Sealed::parse(&decode(&text).unwrap()).unwrap();
    assert_eq!(keyring.open(&travelled, NOW + 120).unwrap().payload.secret.expose(), SECRET);

    let req_text = armor::request_text(&request);
    assert!(req_text.starts_with("Ann is asking for a key."));
    assert_eq!(Request::parse(&decode(&req_text).unwrap()).unwrap().id(), request.id());
    println!("{req_text}\n{text}");
}

/// The mangling corpus, run against one sealed key so the expected bytes are known.
#[test]
fn survives_transit_damage() {
    let (mut keyring, _, sealed) = handoff();
    let text = armor::sealed_text(&sealed, "Ann");
    let body = body_lines(&text);
    let expected = sealed.to_bytes().to_vec();
    let check = |damaged: String, what: &str| {
        let bytes = decode(&damaged).unwrap_or_else(|e| panic!("{what}: {e}\n{damaged}"));
        assert_eq!(bytes, expected, "{what}");
    };

    // Rewrapped: all on one line, and rewrapped at odd widths.
    check(text.replace('\n', " "), "joined into one line");
    let flat = body.join(" ");
    for width in [17, 40, 61] {
        let mut lines = Vec::new();
        let mut line = String::new();
        for group in flat.split(' ') {
            if line.len() + group.len() + 1 > width && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(group);
        }
        lines.push(line);
        check(rebuild(&text, &lines), &format!("rewrapped at {width}"));
    }
    // Email quoting, once and twice, and Windows line endings.
    check(text.lines().map(|l| format!("> {l}")).collect::<Vec<_>>().join("\n"), "quoted");
    check(text.lines().map(|l| format!(">> {l}")).collect::<Vec<_>>().join("\r\n"), "quoted twice, CRLF");
    // Trailing spaces, tabs and blank lines.
    check(text.lines().map(|l| format!("{l}   \t")).collect::<Vec<_>>().join("\n\n"), "trailing whitespace");
    // Chat apps: smart dashes in markers, curly quotes inserted, lowercase.
    check(text.replace("-----", "\u{2014}\u{2014}"), "em-dash markers");
    check(text.replace(' ', " \u{201C}"), "curly quotes inserted");
    check(text.to_lowercase(), "lowercased");
    // Look-alikes typed by hand: 0 as O, 1 as I or L.
    check(text.replace('0', "O").replace('1', "l"), "look-alike characters");
    // Surrounded by chat chatter, missing END marker.
    check(format!("hey here you go:\n{text}\nthanks!"), "surrounded by chat");
    let without_end: String = text.lines().filter(|l| !l.contains("END")).collect::<Vec<_>>().join("\n");
    check(without_end, "END marker cut off");

    // Several characters replaced, spread across the body.
    let chars: usize = body.iter().map(|l| l.replace(' ', "").len()).sum();
    let mut damaged = text.clone();
    for k in 0..8 {
        damaged = corrupt_char(&damaged, (k * 53 + 7) % chars, 'Z');
    }
    check(damaged, "eight replaced characters");
    // Invalid characters (become erasures).
    let mut damaged = text.clone();
    for k in 0..12 {
        damaged = corrupt_char(&damaged, (k * 37 + 3) % chars, '#');
    }
    check(damaged, "twelve invalid characters");
    // A dropped character and an inserted one inside groups.
    let mut lines = body.clone();
    lines[1] = lines[1].replacen(' ', "", 0);
    let mut first: Vec<char> = lines[1].chars().collect();
    first.remove(2);
    lines[1] = first.into_iter().collect();
    lines[3].insert(5, 'Q');
    check(rebuild(&text, &lines), "dropped and inserted characters");
    // Every one of those damaged copies decoded to the same sealed key; it opens once.
    let travelled = Sealed::parse(&expected).unwrap();
    assert_eq!(keyring.open(&travelled, NOW + 120).unwrap().payload.secret.expose(), SECRET);
}

#[test]
fn damage_in_the_preamble_is_repaired() {
    let (_, _, sealed) = handoff();
    let text = armor::sealed_text(&sealed, "Ann");
    // The first 16 characters carry the length; damage two of them.
    let damaged = corrupt_char(&corrupt_char(&text, 0, 'X'), 9, 'X');
    assert_ne!(damaged, text);
    assert_eq!(decode(&damaged).unwrap(), sealed.to_bytes());
}

#[test]
fn too_damaged_is_refused_not_misread() {
    let (_, _, sealed) = handoff();
    let text = armor::sealed_text(&sealed, "Ann");
    let chars: usize = body_lines(&text).iter().map(|l| l.replace(' ', "").len()).sum();
    // Wreck progressively more of the body; every outcome is either the exact
    // object or a clear refusal.
    let mut refused = 0;
    for amount in [40, 80, 160, 300] {
        let mut damaged = text.clone();
        for k in 0..amount {
            damaged = corrupt_char(&damaged, (k * 7919 + 11) % chars, 'W');
        }
        match decode(&damaged) {
            Ok(bytes) => assert_eq!(bytes, sealed.to_bytes(), "misread with {amount} damaged"),
            Err(e) => {
                assert_eq!(e, Error::TooDamaged, "{amount} damaged");
                refused += 1;
            }
        }
    }
    assert!(refused >= 2, "heavy damage must be refused");
    assert_eq!(decode(&body_lines(&text)[..2].join("\n")).unwrap_err(), Error::NotHandoffText);
    let truncated: String = text.lines().take(3).collect::<Vec<_>>().join("\n");
    assert_eq!(decode(&truncated).unwrap_err(), Error::TooDamaged);
}

#[test]
fn rejects_non_objects() {
    assert_eq!(encode(b"not a handoff object", "x").unwrap_err(), Error::Malformed);
    assert_eq!(decode("just a normal chat message").unwrap_err(), Error::NotHandoffText);
}

#[test]
fn size_is_reasonable() {
    let (_, request, sealed) = handoff();
    let req = armor::request_text(&request);
    let key = armor::sealed_text(&sealed, "Ann");
    // Parity (about 1.3x), base32 (1.6x) and group spaces: a few hundred characters.
    assert!(req.len() < 700, "request text is {} chars", req.len());
    assert!(key.len() < 900, "sealed text is {} chars", key.len());
}
