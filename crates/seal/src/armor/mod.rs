//! Armored text for `.handoff` objects: what a `.handoff` file
//! contains and what people paste when a channel can't carry files.
//!
//! ```text
//! Ann is asking for a key. Open this with KeyHandoff.
//! -----BEGIN HANDOFF REQUEST-----
//! 0RQ1 7KQ4 M2PD 9XWR TH3C K8N5
//! ...
//! -----END HANDOFF REQUEST-----
//! ```
//!
//! The body is Crockford base32 in groups of four. Inside it: a 10-byte
//! preamble (armor version, payload length, 7 parity bytes), then the payload
//! plus a 4-byte SHA-256 check, cut into equal Reed-Solomon blocks with about
//! 30% parity and interleaved byte by byte. Reading drops quote markers,
//! curly quotes, dashes and line breaks, treats a group with a bad character
//! or the wrong length as erased, and refuses anything it can't repair
//! exactly. Design: `docs/sealing-format.md#armored-text`.

mod rs;

use sha2::{Digest, Sha256};

use crate::{Card, Error, Kind, Request, Sealed, wire};

const ARMOR_VERSION: u8 = 0;
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const PREAMBLE_DATA: usize = 3;
const PREAMBLE_PARITY: usize = 7;
const PREAMBLE_BYTES: usize = PREAMBLE_DATA + PREAMBLE_PARITY;
/// 10 bytes are exactly 16 base32 characters.
const PREAMBLE_SYMBOLS: usize = PREAMBLE_BYTES * 8 / 5;
const CHECK_BYTES: usize = 4;
const MAX_BLOCK_DATA: usize = 180;
const GROUP: usize = 4;
const GROUPS_PER_LINE: usize = 6;
/// Characters dropped when reading: quoting, chat formatting and dashes.
const IGNORED: &[char] = &['>', '"', '\'', '`', '*', '-', '_', '\u{2010}', '\u{2011}', '\u{2012}', '\u{2013}', '\u{2014}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{00A0}'];

fn marker(kind: Kind) -> &'static str {
    match kind {
        Kind::Request => "HANDOFF REQUEST",
        Kind::Sealed => "HANDOFF SEALED KEY",
        Kind::Card => "HANDOFF CARD",
    }
}

/// Armored text for a request, with the standard plain-language line.
pub fn request_text(request: &Request) -> String {
    let line = format!("{} is asking for a key. Open this with KeyHandoff. It holds no secret.", request.name());
    encode(request.to_bytes(), &line).expect("a parsed request is a valid object")
}

/// Armored text for a contact card, with the standard plain-language line.
pub fn card_text(card: &Card) -> String {
    let line = format!("{}'s KeyHandoff card. Open it in KeyHandoff to send {} keys. It holds no secret.", card.name(), card.name());
    encode(card.to_bytes(), &line).expect("a parsed card is a valid object")
}

/// Armored text for a sealed key addressed to `recipient_name`.
pub fn sealed_text(sealed: &Sealed, recipient_name: &str) -> String {
    let line = format!("A key for {recipient_name}. Only {recipient_name}'s computer can open it, once.");
    encode(sealed.to_bytes(), &line).expect("a parsed sealed key is a valid object")
}

/// Layout of the body after the preamble, derived from the data length.
struct Layout {
    blocks: usize,
    data_per_block: usize,
    parity: usize,
}

impl Layout {
    fn for_len(len: usize) -> Self {
        let blocks = len.div_ceil(MAX_BLOCK_DATA).max(1);
        let data_per_block = len.div_ceil(blocks);
        let parity = (data_per_block * 3).div_ceil(10).max(8).next_multiple_of(2);
        Self { blocks, data_per_block, parity }
    }

    fn block_len(&self) -> usize {
        self.data_per_block + self.parity
    }

    fn total_bytes(&self) -> usize {
        PREAMBLE_BYTES + self.blocks * self.block_len()
    }
}

/// Armor a binary `.handoff` object. `plain_line` is shown above the block
/// for people; reading ignores it.
pub fn encode(object: &[u8], plain_line: &str) -> Result<String, Error> {
    let kind = wire::header(object)?;
    let mut data = object.to_vec();
    data.extend_from_slice(&Sha256::digest(object)[..CHECK_BYTES]);
    let len = u16::try_from(data.len()).map_err(|_| Error::Malformed)?;
    let layout = Layout::for_len(data.len());

    let mut bytes = rs::encode(&[ARMOR_VERSION, (len >> 8) as u8, len as u8], PREAMBLE_PARITY);
    data.resize(layout.blocks * layout.data_per_block, 0);
    let codewords: Vec<Vec<u8>> =
        data.chunks(layout.data_per_block).map(|chunk| rs::encode(chunk, layout.parity)).collect();
    for i in 0..layout.block_len() {
        for cw in &codewords {
            bytes.push(cw[i]);
        }
    }

    let symbols = to_base32(&bytes);
    let mut out = String::new();
    let plain: String = plain_line.chars().filter(|c| !c.is_control()).collect();
    if !plain.trim().is_empty() {
        out.push_str(plain.trim());
        out.push('\n');
    }
    out.push_str(&format!("-----BEGIN {}-----\n", marker(kind)));
    for line in symbols.chunks(GROUP * GROUPS_PER_LINE) {
        let groups: Vec<&str> = line.chunks(GROUP).map(|g| std::str::from_utf8(g).expect("ASCII alphabet")).collect();
        out.push_str(&groups.join(" "));
        out.push('\n');
    }
    out.push_str(&format!("-----END {}-----\n", marker(kind)));
    Ok(out)
}

/// Find the first Handoff message in `text` and return the repaired binary
/// object. Never returns bytes that fail the integrity check.
pub fn decode(text: &str) -> Result<Vec<u8>, Error> {
    // Drop quoting, curly quotes and dashes everywhere and collapse
    // whitespace, so markers are found however a chat app mangled them.
    let upper: String = text.to_uppercase().chars().filter(|c| !IGNORED.contains(c)).collect();
    let upper = upper.split_whitespace().collect::<Vec<_>>().join(" ");
    let (kind, body_start) = [Kind::Request, Kind::Sealed, Kind::Card]
        .into_iter()
        .filter_map(|k| upper.find(&format!("BEGIN {}", marker(k))).map(|at| (k, at + 6 + marker(k).len())))
        .min_by_key(|&(_, at)| at)
        .ok_or(Error::NotHandoffText)?;
    let rest = &upper[body_start..];
    let body = &rest[..rest.find("END HANDOFF").unwrap_or(rest.len())];

    let mut symbols = read_symbols(body);
    if symbols.len() < PREAMBLE_SYMBOLS {
        return Err(Error::TooDamaged);
    }
    let (mut preamble, erased) = symbols_to_bytes(&symbols[..PREAMBLE_SYMBOLS], PREAMBLE_BYTES);
    rs::correct(&mut preamble, PREAMBLE_PARITY, &erased).ok_or(Error::TooDamaged)?;
    if preamble[0] != ARMOR_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    let len = u16::from_be_bytes([preamble[1], preamble[2]]) as usize;
    if len <= CHECK_BYTES + 5 {
        return Err(Error::TooDamaged);
    }
    let layout = Layout::for_len(len);
    let total = layout.total_bytes();
    // Missing characters at the end are erasures; extra ones are ignored.
    symbols.resize((total * 8).div_ceil(5), None);
    let (bytes, erased) = symbols_to_bytes(&symbols, total);

    let n = layout.block_len();
    let mut data = Vec::with_capacity(layout.blocks * layout.data_per_block);
    for b in 0..layout.blocks {
        let positions = (0..n).map(|i| PREAMBLE_BYTES + i * layout.blocks + b);
        let mut cw: Vec<u8> = positions.clone().map(|p| bytes[p]).collect();
        let cw_erased: Vec<usize> = positions.enumerate().filter(|(_, p)| erased.contains(p)).map(|(i, _)| i).collect();
        rs::correct(&mut cw, layout.parity, &cw_erased).ok_or(Error::TooDamaged)?;
        data.extend_from_slice(&cw[..layout.data_per_block]);
    }
    data.truncate(len);
    let (object, check) = data.split_at(len - CHECK_BYTES);
    if Sha256::digest(object)[..CHECK_BYTES] != *check {
        return Err(Error::TooDamaged);
    }
    if wire::header(object)? != kind {
        return Err(Error::Malformed);
    }
    Ok(object.to_vec())
}

fn to_base32(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity((bytes.len() * 8).div_ceil(5));
    let (mut acc, mut bits) = (0u32, 0);
    for &b in bytes {
        acc = (acc << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((acc >> bits) & 31) as usize]);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((acc << (5 - bits)) & 31) as usize]);
    }
    out
}

/// One base32 character, with Crockford's look-alike folding.
fn symbol(c: char) -> Option<u8> {
    let c = match c {
        'O' => '0',
        'I' | 'L' => '1',
        c => c,
    };
    ALPHABET.iter().position(|&a| a as char == c).map(|p| p as u8)
}

/// Turn body text into 5-bit symbols; `None` marks an erased symbol.
fn read_symbols(body: &str) -> Vec<Option<u8>> {
    let tokens: Vec<Vec<char>> = body
        .split_whitespace()
        .map(|t| t.chars().filter(|c| !IGNORED.contains(c)).collect::<Vec<char>>())
        .filter(|t| !t.is_empty())
        .collect();
    let mut out = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        let last = i + 1 == tokens.len();
        if token.len() % GROUP == 0 || (last && token.len() < GROUP) {
            out.extend(token.iter().map(|&c| symbol(c)));
        } else {
            // A group lost or gained a character: we can't know which, so
            // the whole group is erased rather than guessed.
            let groups = ((token.len() + GROUP / 2) / GROUP).max(1);
            out.extend(std::iter::repeat_n(None, groups * GROUP));
        }
    }
    out
}

/// Pack symbols into `count` bytes; a byte is erased if any of its bits is.
fn symbols_to_bytes(symbols: &[Option<u8>], count: usize) -> (Vec<u8>, Vec<usize>) {
    let mut bytes = vec![0u8; count];
    let mut erased = Vec::new();
    for (i, byte) in bytes.iter_mut().enumerate() {
        let mut value = 0u8;
        let mut lost = false;
        for bit in i * 8..i * 8 + 8 {
            match symbols.get(bit / 5).copied().flatten() {
                Some(s) => value = (value << 1) | ((s >> (4 - bit % 5)) & 1),
                None => {
                    value <<= 1;
                    lost = true;
                }
            }
        }
        *byte = value;
        if lost {
            erased.push(i);
        }
    }
    (bytes, erased)
}
