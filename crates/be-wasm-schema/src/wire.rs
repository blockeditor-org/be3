//! What the bytes are: wire types, keys, values as read, and the readers.

use crate::{Error, ErrorKind, MAX_DEPTH};

/// How a value is laid out, which a reader needs to skip it: the key's low four bits. Values 11
/// to 15 are spare, for wire types a later build adds (a reader refuses them as `BadKey`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    /// No bytes: a unit variant, or a message whose fields are all left out.
    Empty = 0,
    Fixed8 = 1,
    Fixed16 = 2,
    Fixed32 = 3,
    Fixed64 = 4,
    Fixed128 = 5,
    /// A block reference: 16 bytes, which `references` finds without the schema.
    Reference = 6,
    /// A 4-byte length, then that many bytes: a string, or bytes.
    Bytes = 7,
    /// A 4-byte length, then fields.
    Message = 8,
    /// A 4-byte length, then the items' wire type (1 byte), their count (4 bytes) and the
    /// items, each as a field's value would be.
    List = 9,
    /// A 4-byte length, then the numbers' type (`Number`, 1 byte) and their little-endian bytes.
    Packed = 10,
}

impl Wire {
    pub fn of(nibble: u8) -> Option<Wire> {
        Some(match nibble {
            0 => Wire::Empty,
            1 => Wire::Fixed8,
            2 => Wire::Fixed16,
            3 => Wire::Fixed32,
            4 => Wire::Fixed64,
            5 => Wire::Fixed128,
            6 => Wire::Reference,
            7 => Wire::Bytes,
            8 => Wire::Message,
            9 => Wire::List,
            10 => Wire::Packed,
            _ => return None,
        })
    }

    /// Whether a value of this wire type has a 4-byte length first.
    pub const fn has_length(self) -> bool {
        matches!(
            self,
            Wire::Bytes | Wire::Message | Wire::List | Wire::Packed
        )
    }

    /// Whether a value of this wire type holds others, and so counts a level of nesting.
    pub const fn nests(self) -> bool {
        matches!(self, Wire::Message | Wire::List)
    }

    /// How many bytes a fixed-size value takes.
    pub const fn fixed_size(self) -> Option<usize> {
        match self {
            Wire::Empty => Some(0),
            Wire::Fixed8 => Some(1),
            Wire::Fixed16 => Some(2),
            Wire::Fixed32 => Some(4),
            Wire::Fixed64 => Some(8),
            Wire::Fixed128 | Wire::Reference => Some(16),
            _ => None,
        }
    }
}

/// The type of packed numbers: the byte after a packed list's length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Number {
    I8 = 1,
    U16 = 2,
    I16 = 3,
    U32 = 4,
    I32 = 5,
    U64 = 6,
    I64 = 7,
    U128 = 8,
    I128 = 9,
    F32 = 10,
    F64 = 11,
    Bool = 12,
    Char = 13,
}

impl Number {
    pub fn of(byte: u8) -> Option<Number> {
        Some(match byte {
            1 => Number::I8,
            2 => Number::U16,
            3 => Number::I16,
            4 => Number::U32,
            5 => Number::I32,
            6 => Number::U64,
            7 => Number::I64,
            8 => Number::U128,
            9 => Number::I128,
            10 => Number::F32,
            11 => Number::F64,
            12 => Number::Bool,
            13 => Number::Char,
            _ => return None,
        })
    }

    pub const fn size(self) -> usize {
        match self {
            Number::I8 | Number::Bool => 1,
            Number::U16 | Number::I16 => 2,
            Number::U32 | Number::I32 | Number::F32 | Number::Char => 4,
            Number::U64 | Number::I64 | Number::F64 => 8,
            Number::U128 | Number::I128 => 16,
        }
    }
}

/// A field's id and its two flags: `critical`, a reader that does not know the field must
/// refuse the value rather than skip it; `no_copy`, a reader that does not know it may skip it
/// but must not write it back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub id: u32,
    pub critical: bool,
    pub no_copy: bool,
}

impl Key {
    pub const fn new(id: u32) -> Self {
        Key {
            id,
            critical: false,
            no_copy: false,
        }
    }

    pub const fn flagged(id: u32, critical: bool, no_copy: bool) -> Self {
        Key {
            id,
            critical,
            no_copy,
        }
    }

    pub(crate) fn bits(self, wire: Wire) -> u64 {
        (u64::from(self.id) << 6)
            | (u64::from(self.no_copy) << 5)
            | (u64::from(self.critical) << 4)
            | wire as u64
    }
}

/// A block reference: the id of a block, which `references` finds wherever it is in a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Reference(pub [u8; 16]);

/// A message's fields, and how many levels may still nest below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nested<'de> {
    pub bytes: &'de [u8],
    pub(crate) depth: u32,
}

/// A list's items: their wire type, how many, and their bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Items<'de> {
    pub wire: Wire,
    pub count: u32,
    /// The whole contents: the header and the items.
    pub(crate) payload: &'de [u8],
    pub(crate) depth: u32,
}

/// Packed numbers: their type and little-endian bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packed<'de> {
    pub number: Number,
    pub bytes: &'de [u8],
}

/// A value as read, before its type decodes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value<'de> {
    Empty,
    Fixed8(u8),
    Fixed16(u16),
    Fixed32(u32),
    Fixed64(u64),
    Fixed128(u128),
    Reference(Reference),
    Bytes(&'de [u8]),
    Message(Nested<'de>),
    List(Items<'de>),
    Packed(Packed<'de>),
}

impl<'de> Value<'de> {
    pub fn wire(&self) -> Wire {
        match self {
            Value::Empty => Wire::Empty,
            Value::Fixed8(_) => Wire::Fixed8,
            Value::Fixed16(_) => Wire::Fixed16,
            Value::Fixed32(_) => Wire::Fixed32,
            Value::Fixed64(_) => Wire::Fixed64,
            Value::Fixed128(_) => Wire::Fixed128,
            Value::Reference(_) => Wire::Reference,
            Value::Bytes(_) => Wire::Bytes,
            Value::Message(_) => Wire::Message,
            Value::List(_) => Wire::List,
            Value::Packed(_) => Wire::Packed,
        }
    }

    pub(crate) fn mismatch(&self, expected: Wire) -> Error {
        Error::new(ErrorKind::Wire {
            expected,
            found: self.wire(),
        })
    }

    /// The outermost value: `bytes` read as a value of wire type `wire`, as `encode` wrote it
    /// (a length-prefixed value without its length).
    pub fn outermost(wire: Wire, bytes: &'de [u8]) -> Result<Self, Error> {
        if wire.has_length() {
            return Value::of_contents(wire, bytes, MAX_DEPTH);
        }
        let mut reader = Reader::new(bytes);
        let value = reader.value(wire)?;
        if !reader.is_empty() {
            return Err(Error::new(ErrorKind::Trailing(reader.remaining())));
        }
        Ok(value)
    }

    /// A length-prefixed value from its contents.
    pub(crate) fn of_contents(wire: Wire, bytes: &'de [u8], depth: u32) -> Result<Self, Error> {
        Ok(match wire {
            Wire::Bytes => Value::Bytes(bytes),
            Wire::Message => Value::Message(Nested { bytes, depth }),
            Wire::List => {
                let header = bytes.get(..5).ok_or(Error::new(ErrorKind::Truncated))?;
                let wire = Wire::of(header[0]).ok_or(Error::new(ErrorKind::BadList))?;
                let count = u32::from_le_bytes(header[1..5].try_into().expect("four bytes"));
                Value::List(Items {
                    wire,
                    count,
                    payload: bytes,
                    depth,
                })
            }
            Wire::Packed => {
                let (&number, bytes) = bytes
                    .split_first()
                    .ok_or(Error::new(ErrorKind::Truncated))?;
                let number = Number::of(number).ok_or(Error::new(ErrorKind::BadList))?;
                if bytes.len() % number.size() != 0 {
                    return Err(Error::new(ErrorKind::Truncated));
                }
                Value::Packed(Packed { number, bytes })
            }
            _ => unreachable!("only length-prefixed wire types have contents"),
        })
    }

    pub fn bytes(self) -> Result<&'de [u8], Error> {
        match self {
            Value::Bytes(bytes) => Ok(bytes),
            other => Err(other.mismatch(Wire::Bytes)),
        }
    }

    pub fn packed(self) -> Result<Packed<'de>, Error> {
        match self {
            Value::Packed(packed) => Ok(packed),
            other => Err(other.mismatch(Wire::Packed)),
        }
    }

    /// The value as an unsigned number of at most 128 bits, from any fixed width.
    pub fn unsigned(self) -> Result<u128, Error> {
        Ok(match self {
            Value::Fixed8(value) => value.into(),
            Value::Fixed16(value) => value.into(),
            Value::Fixed32(value) => value.into(),
            Value::Fixed64(value) => value.into(),
            Value::Fixed128(value) => value,
            other => return Err(other.mismatch(Wire::Fixed64)),
        })
    }

    /// The value as a signed number, its fixed width sign-extended.
    pub fn signed(self) -> Result<i128, Error> {
        Ok(match self {
            Value::Fixed8(value) => (value as i8).into(),
            Value::Fixed16(value) => (value as i16).into(),
            Value::Fixed32(value) => (value as i32).into(),
            Value::Fixed64(value) => (value as i64).into(),
            Value::Fixed128(value) => value as i128,
            other => return Err(other.mismatch(Wire::Fixed64)),
        })
    }
}

/// Reads the fields of a message, borrowing from it.
pub struct Reader<'de> {
    bytes: &'de [u8],
    at: usize,
    /// How many levels the values it reads may still nest.
    depth: u32,
}

impl<'de> Reader<'de> {
    /// Reads `bytes` as the outermost message.
    pub fn new(bytes: &'de [u8]) -> Self {
        Reader {
            bytes,
            at: 0,
            depth: MAX_DEPTH,
        }
    }

    /// Reads inside `value`, a message (or nothing, a message with no fields): a level deeper.
    pub fn of(value: Value<'de>) -> Result<Self, Error> {
        match value {
            Value::Message(nested) => Ok(Reader {
                bytes: nested.bytes,
                at: 0,
                depth: nested
                    .depth
                    .checked_sub(1)
                    .ok_or(Error::new(ErrorKind::TooDeep))?,
            }),
            Value::Empty => Ok(Reader {
                bytes: &[],
                at: 0,
                depth: 0,
            }),
            other => Err(other.mismatch(Wire::Message)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.at == self.bytes.len()
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    fn take(&mut self, length: usize) -> Result<&'de [u8], Error> {
        let end = self
            .at
            .checked_add(length)
            .ok_or(Error::new(ErrorKind::Truncated))?;
        let bytes = self
            .bytes
            .get(self.at..end)
            .ok_or(Error::new(ErrorKind::Truncated))?;
        self.at = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        Ok(self.take(N)?.try_into().expect("N bytes"))
    }

    fn key(&mut self) -> Result<u64, Error> {
        let mut key = 0u64;
        // An id is at most 32 bits, so a key at most 38: six bytes of seven.
        for shift in (0..42).step_by(7) {
            let byte = *self
                .bytes
                .get(self.at)
                .ok_or(Error::new(ErrorKind::Truncated))?;
            self.at += 1;
            key |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(key);
            }
        }
        Err(Error::new(ErrorKind::BadKey))
    }

    /// A value of wire type `wire`, as a field's value or a list's item is written.
    pub fn value(&mut self, wire: Wire) -> Result<Value<'de>, Error> {
        Ok(match wire {
            Wire::Empty => Value::Empty,
            Wire::Fixed8 => Value::Fixed8(self.array::<1>()?[0]),
            Wire::Fixed16 => Value::Fixed16(u16::from_le_bytes(self.array()?)),
            Wire::Fixed32 => Value::Fixed32(u32::from_le_bytes(self.array()?)),
            Wire::Fixed64 => Value::Fixed64(u64::from_le_bytes(self.array()?)),
            Wire::Fixed128 => Value::Fixed128(u128::from_le_bytes(self.array()?)),
            Wire::Reference => Value::Reference(Reference(self.array()?)),
            _ => {
                let length = u32::from_le_bytes(self.array()?) as usize;
                let contents = self.take(length)?;
                Value::of_contents(wire, contents, self.depth)?
            }
        })
    }

    /// The next field of a message: its key and value.
    pub fn field(&mut self) -> Result<(Key, Value<'de>), Error> {
        let key = self.key()?;
        let wire = Wire::of((key & 0xf) as u8).ok_or(Error::new(ErrorKind::BadKey))?;
        let id = u32::try_from(key >> 6).map_err(|_| Error::new(ErrorKind::BadKey))?;
        if id == 0 {
            return Err(Error::new(ErrorKind::BadKey));
        }
        let key = Key {
            id,
            critical: key & 0x10 != 0,
            no_copy: key & 0x20 != 0,
        };
        Ok((key, self.value(wire)?))
    }
}

/// Reads the items of a list in turn.
pub struct ItemsReader<'de> {
    reader: Reader<'de>,
    wire: Wire,
    left: u32,
    count: u32,
}

impl<'de> ItemsReader<'de> {
    /// The items of `value`, a list of `wire` items: a level deeper.
    pub fn of(value: Value<'de>, wire: Wire) -> Result<Self, Error> {
        let Value::List(items) = value else {
            return Err(value.mismatch(Wire::List));
        };
        if items.wire != wire {
            return Err(Error::new(ErrorKind::BadList));
        }
        Self::any(items)
    }

    /// The items of a list, whatever their wire type.
    pub fn any(items: Items<'de>) -> Result<Self, Error> {
        let depth = items
            .depth
            .checked_sub(1)
            .ok_or(Error::new(ErrorKind::TooDeep))?;
        // Each item but an empty one takes at least a byte, so a count past what the bytes
        // could hold is refused before anything is allocated for it.
        let bytes = &items.payload[5..];
        if items.wire != Wire::Empty && items.count as usize > bytes.len() {
            return Err(Error::new(ErrorKind::BadList));
        }
        Ok(ItemsReader {
            reader: Reader {
                bytes,
                at: 0,
                depth,
            },
            wire: items.wire,
            left: items.count,
            count: items.count,
        })
    }

    pub fn len(&self) -> usize {
        self.count as usize
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn wire(&self) -> Wire {
        self.wire
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Result<Value<'de>, Error>> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        Some(self.reader.value(self.wire))
    }

    /// Refuses bytes after the last item.
    pub fn finish(self) -> Result<(), Error> {
        if self.reader.is_empty() {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::BadList))
        }
    }
}
