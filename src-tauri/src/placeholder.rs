//! Placeholders: what a program started by `keyhandoff run` holds instead
//! of a key whose server is known. A placeholder keeps the key's prefix and
//! length, so client libraries accept it and requests are the same size; the
//! TLS hook swaps the real key in only on a connection whose certificate is
//! valid for one of the key's hosts.

use rand_core::{OsRng, RngCore};
use zeroize::Zeroizing;

/// Where well-known keys are sent, by prefix. A key whose provider signs
/// requests instead of sending the key (AWS) has no entry: a placeholder
/// can't work for it.
const KNOWN: &[(&str, &str)] = &[
    ("sk_live_", "api.stripe.com"),
    ("sk_test_", "api.stripe.com"),
    ("rk_live_", "api.stripe.com"),
    ("rk_test_", "api.stripe.com"),
    ("sk-ant-", "api.anthropic.com"),
    ("sk-proj-", "api.openai.com"),
    ("sk-", "api.openai.com"),
    ("github_pat_", "api.github.com"),
    ("ghp_", "api.github.com"),
    ("gho_", "api.github.com"),
    ("ghu_", "api.github.com"),
    ("ghs_", "api.github.com"),
    ("glpat-", "gitlab.com"),
    ("xoxb-", "slack.com"),
    ("xoxp-", "slack.com"),
    ("hf_", "huggingface.co"),
    ("SG.", "api.sendgrid.com"),
];

/// The hosts a key goes to, guessed from its prefix; empty when unknown.
pub fn known_hosts(key: &[u8]) -> Vec<String> {
    KNOWN.iter().find(|(prefix, _)| key.starts_with(prefix.as_bytes())).map(|(_, host)| vec![host.to_string()]).unwrap_or_default()
}

/// A host as KeyHandoff keeps it: lowercase letters, digits, dots and
/// hyphens, without a scheme, path or port. `None` if it isn't one.
pub fn clean_host(host: &str) -> Option<String> {
    let host = host.trim().trim_start_matches("https://").trim_start_matches("http://");
    let host = host.split(['/', ':']).next()?.trim_end_matches('.').to_ascii_lowercase();
    let ok = !host.is_empty()
        && host.len() <= 253
        && host.contains('.')
        && host.split('.').all(|label| !label.is_empty() && label.len() <= 63 && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
    ok.then_some(host)
}

/// A stand-in for `key`: the same length, the same prefix (a known one, or
/// whatever comes before the last `_` or `-` in the first 16 characters), and
/// random characters of the same kinds after it. `None` for a key that isn't
/// printable ASCII, which a placeholder couldn't imitate.
pub fn placeholder(key: &[u8]) -> Option<Zeroizing<String>> {
    if key.is_empty() || !key.iter().all(|b| b.is_ascii_graphic()) {
        return None;
    }
    let prefix = prefix_len(key);
    let body = &key[prefix..];
    let alphabet: Vec<u8> = if body.iter().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) {
        b"0123456789abcdef".to_vec()
    } else {
        let mut a = Vec::new();
        if body.iter().any(u8::is_ascii_uppercase) {
            a.extend(b'A'..=b'Z');
        }
        if body.iter().any(u8::is_ascii_lowercase) {
            a.extend(b'a'..=b'z');
        }
        if body.iter().any(u8::is_ascii_digit) || a.is_empty() {
            a.extend(b'0'..=b'9');
        }
        a
    };
    loop {
        let mut out = Zeroizing::new(String::with_capacity(key.len()));
        out.push_str(std::str::from_utf8(&key[..prefix]).ok()?);
        for _ in body {
            out.push(alphabet[(OsRng.next_u32() as usize) % alphabet.len()] as char);
        }
        if out.as_bytes() != key {
            return Some(out);
        }
    }
}

fn prefix_len(key: &[u8]) -> usize {
    if let Some((prefix, _)) = KNOWN.iter().find(|(prefix, _)| key.starts_with(prefix.as_bytes())) {
        return prefix.len();
    }
    key.iter().take(16).rposition(|&b| b == b'_' || b == b'-').map(|i| i + 1).filter(|&n| n < key.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_look_like_the_key_and_hosts_are_known_by_prefix() {
        let key = b"sk_test_51SyntheticPlaceholderKey0123456789";
        let p = placeholder(key).unwrap();
        assert_eq!(p.len(), key.len());
        assert!(p.starts_with("sk_test_") && p.as_bytes() != key);
        assert_ne!(placeholder(key).unwrap(), p, "fresh each time");
        assert_eq!(known_hosts(key), ["api.stripe.com"]);
        assert_eq!(known_hosts(b"sk-ant-api03-synthetic"), ["api.anthropic.com"]);
        assert!(known_hosts(b"AKIASYNTHETIC").is_empty());
        let hex = placeholder(b"dp_0123456789abcdef").unwrap();
        assert!(hex.starts_with("dp_") && hex[3..].bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(placeholder("ключ".as_bytes()).is_none());
        assert_eq!(clean_host("https://API.Stripe.com/v1").as_deref(), Some("api.stripe.com"));
        assert_eq!(clean_host("localhost"), None);
    }
}
