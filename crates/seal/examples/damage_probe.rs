//! How many random character replacements a sealed key survives.
use keyhandoff_seal::{armor, create_request, seal, Identity, Payload, Secret};
fn main() {
    let me = Identity::generate();
    let (req, _) = create_request(&me, "Ann", 0, 100).unwrap();
    let p = Payload { label: "Anthropic".into(), note: String::new(), secret: Secret::new(vec![b'k'; 108]) };
    let sealed = seal(&me, &req, &p, 1).unwrap();
    let text = armor::sealed_text(&sealed, "Ann");
    let begin = text.find("-----\n").unwrap() + 6;
    let end = text.rfind("-----END").unwrap();
    let idx: Vec<usize> = (begin..end).filter(|&i| text.as_bytes()[i] != b' ' && text.as_bytes()[i] != b'\n').collect();
    let mut seed = 7u64;
    let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
    for n in [5, 10, 20, 30, 40, 60] {
        let (mut ok, mut refused, mut wrong) = (0, 0, 0);
        for _ in 0..300 {
            let mut b = text.clone().into_bytes();
            for _ in 0..n { let i = idx[(rnd() % idx.len() as u64) as usize]; b[i] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ"[(rnd() % 32) as usize]; }
            match armor::decode(&String::from_utf8(b).unwrap()) { Ok(v) if v == sealed.to_bytes() => ok += 1, Ok(_) => wrong += 1, Err(_) => refused += 1 }
        }
        println!("{n:>3} of {} chars replaced: opened {ok}/300, refused {refused}, misread {wrong}", idx.len());
    }
}
