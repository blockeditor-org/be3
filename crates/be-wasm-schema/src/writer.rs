//! Writing values: into bytes, or only counting them (`encoded_len`).

use crate::wire::{Key, Wire};
use crate::{Encode, Error, ErrorKind, MAX_DEPTH};

/// Where encoding writes: bytes, or, counting, only how many there would be. Every byte goes
/// where it ends up; a length is written as a placeholder and set once its contents are.
pub struct Writer {
    bytes: Option<Vec<u8>>,
    len: usize,
    /// How many messages and lists the value being written is inside.
    depth: u32,
}

impl Writer {
    pub fn new() -> Self {
        Writer {
            bytes: Some(Vec::new()),
            len: 0,
            depth: 0,
        }
    }

    /// The writer for the outermost value, of wire type `wire`: a level deep already if it is
    /// a message or a list, as decoding counts it.
    pub(crate) fn outermost(mut self, wire: Wire) -> Self {
        if wire.nests() {
            self.depth = 1;
        }
        self
    }

    /// A writer that only counts.
    pub fn counting() -> Self {
        Writer {
            bytes: None,
            len: 0,
            depth: 0,
        }
    }

    /// Another writer into bytes, as deep as this one: for parts written apart to be sorted
    /// (map entries, set elements).
    pub fn apart(&self) -> Self {
        Writer {
            bytes: Some(Vec::new()),
            len: 0,
            depth: self.depth,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes.unwrap_or_default()
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        if let Some(out) = &mut self.bytes {
            out.extend_from_slice(bytes);
        }
        self.len += bytes.len();
    }

    pub fn key(&mut self, key: Key, wire: Wire) {
        let mut bits = key.bits(wire);
        while bits >= 0x80 {
            self.bytes(&[bits as u8 | 0x80]);
            bits >>= 7;
        }
        self.bytes(&[bits as u8]);
    }

    /// Writes a length, `body`, then sets the length to what `body` wrote; a level deeper when
    /// `wire` nests. Answers the contents' length.
    pub fn length(
        &mut self,
        wire: Wire,
        body: impl FnOnce(&mut Self) -> Result<(), Error>,
    ) -> Result<usize, Error> {
        if wire.nests() {
            if self.depth >= MAX_DEPTH {
                return Err(Error::new(ErrorKind::TooDeep));
            }
            self.depth += 1;
        }
        let at = self.len;
        self.bytes(&[0; 4]);
        body(self)?;
        if wire.nests() {
            self.depth -= 1;
        }
        let length = self.len - at - 4;
        let prefix = u32::try_from(length).map_err(|_| Error::new(ErrorKind::TooLarge))?;
        if let Some(out) = &mut self.bytes {
            out[at..at + 4].copy_from_slice(&prefix.to_le_bytes());
        }
        Ok(length)
    }

    /// `value` as a list item or a field's value: with its length first if it has one.
    pub fn value<T: Encode + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        if T::WIRE.has_length() {
            self.length(T::WIRE, |w| value.encode(w)).map(drop)
        } else {
            value.encode(self)
        }
    }

    /// `value` as the field `key` names, even when it is absent (an enum's variant, or what
    /// `Some` holds). A message with no fields is written as nothing (`Wire::Empty`).
    pub fn field<T: Encode + ?Sized>(&mut self, key: Key, value: &T) -> Result<(), Error> {
        if T::WIRE == Wire::Message {
            return self.message(key, |w| value.encode(w));
        }
        self.key(key, T::WIRE);
        self.value(value)
    }

    /// The field `key` names, holding a message `body` writes, or nothing when it writes no
    /// fields.
    pub fn message(
        &mut self,
        key: Key,
        body: impl FnOnce(&mut Self) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let key_at = self.len;
        self.key(key, Wire::Message);
        if self.length(Wire::Message, body)? == 0 {
            // The wire type is the key's low four bits, in its first byte.
            let end = self.len - 4;
            self.len = end;
            if let Some(out) = &mut self.bytes {
                out.truncate(end);
                out[key_at] = (out[key_at] & 0xf0) | Wire::Empty as u8;
            }
        }
        Ok(())
    }

    /// The field `key` names, holding nothing: a unit variant.
    pub fn empty(&mut self, key: Key) {
        self.key(key, Wire::Empty);
    }

    /// A list's header: its items' wire type and how many there are.
    pub fn list_header(&mut self, wire: Wire, count: usize) -> Result<(), Error> {
        let count = u32::try_from(count).map_err(|_| Error::new(ErrorKind::TooLarge))?;
        self.bytes(&[wire as u8]);
        self.bytes(&count.to_le_bytes());
        Ok(())
    }
}

impl Default for Writer {
    fn default() -> Self {
        Writer::new()
    }
}
