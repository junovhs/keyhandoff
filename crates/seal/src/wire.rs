//! Byte-level reading and writing shared by the object formats.

use crate::{Error, FORMAT_VERSION, Kind, MAGIC};

pub(crate) fn header(bytes: &[u8]) -> Result<Kind, Error> {
    if bytes.len() < 5 || &bytes[..3] != MAGIC {
        return Err(Error::Malformed);
    }
    if bytes[3] != FORMAT_VERSION {
        return Err(Error::UnsupportedVersion);
    }
    Kind::from_byte(bytes[4]).ok_or(Error::Malformed)
}

pub(crate) fn write_header(out: &mut Vec<u8>, kind: Kind) {
    out.extend_from_slice(MAGIC);
    out.push(FORMAT_VERSION);
    out.push(kind as u8);
}

/// A text field: non-empty, at most `max` UTF-8 bytes, no control characters.
pub(crate) fn valid_text(s: &str, max: usize, allow_empty: bool) -> bool {
    (allow_empty || !s.is_empty()) && s.len() <= max && !s.chars().any(char::is_control)
}

/// Cursor over a byte slice; every read fails with `Malformed` when short.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8], pos: usize) -> Self {
        Self { bytes, pos }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).ok_or(Error::Malformed)?;
        let slice = self.bytes.get(self.pos..end).ok_or(Error::Malformed)?;
        self.pos = end;
        Ok(slice)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        Ok(self.take(N)?.try_into().expect("take returned N bytes"))
    }

    pub(crate) fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    pub(crate) fn text(&mut self, len: usize, max: usize, allow_empty: bool) -> Result<String, Error> {
        let s = std::str::from_utf8(self.take(len)?).map_err(|_| Error::Malformed)?;
        if !valid_text(s, max, allow_empty) {
            return Err(Error::Malformed);
        }
        Ok(s.to_owned())
    }

    pub(crate) fn finish(&self) -> Result<(), Error> {
        if self.pos == self.bytes.len() { Ok(()) } else { Err(Error::Malformed) }
    }
}
