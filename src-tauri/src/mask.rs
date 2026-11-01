//! Hiding keys in what a program prints. `keyhandoff run` passes the
//! program's stdout and stderr through a [`Masker`], so a key printed plain,
//! base64, hex, URL-encoded or reversed comes out as
//! `[KeyHandoff: Stripe hidden]`, even when it arrives a few bytes at a time.
//!
//! Any run of [`FRAGMENT`] or more bytes copied from one of those forms is
//! hidden too, so printing most of a key, or a key cut in two, doesn't get
//! past. A shorter fragment, such as a provider's `sk_live_` prefix, passes.
//! This catches a key printed by accident or by a careless tool; code that
//! wants the key out can encode it some other way, which is why FEAT-09 keeps
//! the key out of the program altogether.

use std::collections::HashSet;

use zeroize::Zeroizing;

/// The shortest run of a key's bytes that's hidden.
pub const FRAGMENT: usize = 12;

struct Form {
    label: String,
    bytes: Zeroizing<Vec<u8>>,
}

/// Streams output and hides every printed form of the keys it was given.
pub struct Masker {
    forms: Vec<Form>,
    /// Every `FRAGMENT`-byte window of every form (or a whole form shorter than that).
    starts: HashSet<Vec<u8>>,
    shortest: usize,
    /// Bytes held back because they might be the start of a key.
    held: Zeroizing<Vec<u8>>,
}

impl Masker {
    /// `keys`: each key's display name and value.
    pub fn new<'a>(keys: impl IntoIterator<Item = (&'a str, &'a [u8])>) -> Self {
        let mut forms = Vec::new();
        for (label, value) in keys {
            if value.is_empty() {
                continue;
            }
            for bytes in forms_of(value) {
                forms.push(Form { label: label.to_string(), bytes });
            }
        }
        let mut starts = HashSet::new();
        for form in &forms {
            let w = FRAGMENT.min(form.bytes.len());
            for window in form.bytes.windows(w) {
                starts.insert(window.to_vec());
            }
        }
        let shortest = forms.iter().map(|f| FRAGMENT.min(f.bytes.len())).min().unwrap_or(FRAGMENT);
        Self { forms, starts, shortest, held: Zeroizing::new(Vec::new()) }
    }

    /// Masked output for the next chunk. Bytes that could still turn into a
    /// key are held until the next chunk or [`finish`](Self::finish).
    pub fn push(&mut self, chunk: &[u8]) -> Vec<u8> {
        self.held.extend_from_slice(chunk);
        self.drain(false)
    }

    /// Whatever is still held, masked.
    pub fn finish(&mut self) -> Vec<u8> {
        self.drain(true)
    }

    fn drain(&mut self, last: bool) -> Vec<u8> {
        if self.forms.is_empty() {
            return std::mem::take(&mut *self.held);
        }
        let buf = std::mem::take(&mut *self.held);
        let mut out = Vec::with_capacity(buf.len());
        let mut i = 0;
        while i < buf.len() {
            if let Some(w) = (self.shortest..=FRAGMENT).rev().find(|&w| i + w <= buf.len() && self.starts.contains(&buf[i..i + w])) {
                // Grow the match while it's still part of one form.
                let mut end = i + w;
                while end < buf.len() && self.within(&buf[i..=end]).is_some() {
                    end += 1;
                }
                if end == buf.len() && !last {
                    break; // It might go on in the next chunk.
                }
                let label = self.within(&buf[i..end]).unwrap_or("A key");
                out.extend_from_slice(format!("[KeyHandoff: {label} hidden]").as_bytes());
                i = end;
                continue;
            }
            if !last && buf.len() - i < FRAGMENT && self.within(&buf[i..]).is_some() {
                break; // Too short to judge yet.
            }
            out.push(buf[i]);
            i += 1;
        }
        self.held.extend_from_slice(&buf[i..]);
        let mut buf = buf;
        zeroize::Zeroize::zeroize(&mut buf);
        out
    }

    /// The label of a form that contains `bytes`.
    fn within(&self, bytes: &[u8]) -> Option<&str> {
        self.forms.iter().find(|f| contains(&f.bytes, bytes)).map(|f| f.label.as_str())
    }
}

impl Drop for Masker {
    fn drop(&mut self) {
        for mut window in self.starts.drain() {
            zeroize::Zeroize::zeroize(&mut window);
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.len() <= haystack.len() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// The ways a key gets printed: plain, reversed, hex, base64 (standard and
/// URL-safe, at each of the three byte offsets it can start at inside longer
/// data) and percent-encoded.
fn forms_of(key: &[u8]) -> Vec<Zeroizing<Vec<u8>>> {
    let mut forms = vec![Zeroizing::new(key.to_vec()), Zeroizing::new(key.iter().rev().copied().collect())];
    let hex: Vec<u8> = key.iter().flat_map(|b| format!("{b:02x}").into_bytes()).collect();
    forms.push(Zeroizing::new(hex.to_ascii_uppercase()));
    forms.push(Zeroizing::new(hex));
    for alphabet in [STANDARD, URL_SAFE] {
        for offset in 0..3 {
            // Base64 of the key after `offset` other bytes: skip the characters
            // that depend on those bytes or on whatever follows the key.
            let mut padded = vec![0u8; offset];
            padded.extend_from_slice(key);
            let encoded = base64(&padded, alphabet);
            let skip = if offset == 0 { 0 } else { (offset * 8).div_ceil(6) };
            let keep = (padded.len() * 8) / 6;
            if keep > skip {
                forms.push(Zeroizing::new(encoded.as_bytes()[skip..keep].to_vec()));
            }
        }
    }
    for upper in [true, false] {
        let encoded: Vec<u8> = key
            .iter()
            .flat_map(|&b| if upper { format!("%{b:02X}") } else { format!("%{b:02x}") }.into_bytes())
            .collect();
        forms.push(Zeroizing::new(encoded));
    }
    // URL-encoding that leaves letters, digits and -._~ alone.
    let unreserved = |b: u8| b.is_ascii_alphanumeric() || b"-._~".contains(&b);
    if !key.iter().all(|&b| unreserved(b)) {
        let encoded: Vec<u8> = key.iter().flat_map(|&b| if unreserved(b) { vec![b] } else { format!("%{b:02X}").into_bytes() }).collect();
        forms.push(Zeroizing::new(encoded));
    }
    forms.sort_by(|a, b| a[..].cmp(&b[..]));
    forms.dedup_by(|a, b| a[..] == b[..]);
    forms
}

const STANDARD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL_SAFE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn base64(data: &[u8], alphabet: &[u8; 64]) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::with_capacity(data.len().div_ceil(3) * 4));
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(alphabet[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8] = b"sk_test_51SyntheticKeyForMaskingTests0123";

    fn masked(input: &[u8], step: usize) -> String {
        let mut m = Masker::new([("Stripe", KEY)]);
        let mut out = Vec::new();
        for chunk in input.chunks(step) {
            out.extend(m.push(chunk));
        }
        out.extend(m.finish());
        String::from_utf8(out).unwrap()
    }

    fn b64(data: &[u8]) -> String {
        base64(data, STANDARD).to_string()
    }

    #[test]
    fn every_printed_form_is_hidden_whatever_the_chunking() {
        let hex: String = KEY.iter().map(|b| format!("{b:02x}")).collect();
        let reversed: String = String::from_utf8(KEY.iter().rev().copied().collect()).unwrap();
        let url: String = KEY.iter().map(|b| format!("%{b:02X}")).collect();
        let key = std::str::from_utf8(KEY).unwrap();
        let forms = [
            key.to_string(),
            b64(KEY),
            b64(&[b"x".as_slice(), KEY].concat()),
            b64(&[b"Bearer: ".as_slice(), KEY, b"\n"].concat()),
            hex.clone(),
            hex.to_uppercase(),
            url,
            reversed,
            key[..30].to_string(),
            format!("{} {}", &key[..20], &key[20..]),
        ];
        for form in &forms {
            for step in [1, 3, 7, 4096] {
                let out = masked(format!("token={form} done\n").as_bytes(), step);
                assert!(out.contains("[KeyHandoff: Stripe hidden]"), "{form} at {step}: {out}");
                assert!(out.starts_with("token=") && out.ends_with(" done\n"), "{out}");
                for window in KEY.windows(FRAGMENT) {
                    assert!(!out.contains(std::str::from_utf8(window).unwrap()), "{form} at {step} leaked: {out}");
                }
            }
        }
    }

    #[test]
    fn ordinary_output_and_a_short_prefix_pass_unchanged() {
        let text = "Charging card with sk_test_ key...\nPrompt? ";
        assert_eq!(masked(text.as_bytes(), 1), text);
        // A prompt that ends in a fragment of a key is held only until more comes.
        let mut m = Masker::new([("Stripe", KEY)]);
        assert_eq!(m.push(b"Enter > "), b"Enter > ");
        assert_eq!(m.push(b"sk_"), b"");
        assert_eq!(m.push(b"!"), b"sk_!");
    }
}
