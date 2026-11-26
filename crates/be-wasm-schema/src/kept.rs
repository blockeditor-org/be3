//! Values kept as bytes: unknown fields and variants, and `Kept<T>`.

use std::fmt;
use std::marker::PhantomData;

use crate::wire::{Key, Reader, Value, Wire};
use crate::{
    BUDGET_BASE, BUDGET_PER_BYTE, Budget, Decode, Encode, Error, ErrorKind, MAX_DEPTH, Writer,
    charge,
};

/// A value kept as it was read, to be written back unchanged: its wire type and contents (its
/// fixed bytes, or what follows its length).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raw {
    wire: Wire,
    contents: Vec<u8>,
}

impl Raw {
    pub fn of(value: Value<'_>) -> Result<Raw, Error> {
        let contents = match value {
            Value::Empty => Vec::new(),
            Value::Fixed8(value) => vec![value],
            Value::Fixed16(value) => value.to_le_bytes().to_vec(),
            Value::Fixed32(value) => value.to_le_bytes().to_vec(),
            Value::Fixed64(value) => value.to_le_bytes().to_vec(),
            Value::Fixed128(value) => value.to_le_bytes().to_vec(),
            Value::Reference(reference) => reference.0.to_vec(),
            Value::Bytes(bytes) => bytes.to_vec(),
            Value::Message(nested) => nested.bytes.to_vec(),
            Value::List(items) => items.payload.to_vec(),
            Value::Packed(packed) => {
                let mut contents = vec![packed.number as u8];
                contents.extend_from_slice(packed.bytes);
                contents
            }
        };
        charge(contents.len() as u64 + size_of::<Raw>() as u64)?;
        Ok(Raw {
            wire: value.wire(),
            contents,
        })
    }

    pub fn wire(&self) -> Wire {
        self.wire
    }

    /// The value again, as it was read.
    pub fn value(&self) -> Result<Value<'_>, Error> {
        if self.wire.has_length() {
            Value::of_contents(self.wire, &self.contents, MAX_DEPTH)
        } else {
            let mut reader = Reader::new(&self.contents);
            reader.value(self.wire)
        }
    }

    fn write_value(&self, w: &mut Writer) -> Result<(), Error> {
        if self.wire.has_length() {
            w.length(self.wire, |w| {
                w.bytes(&self.contents);
                Ok(())
            })
            .map(drop)
        } else {
            w.bytes(&self.contents);
            Ok(())
        }
    }

    fn write_field(&self, id: u32, w: &mut Writer) -> Result<(), Error> {
        w.key(Key::new(id), self.wire);
        self.write_value(w)
    }
}

/// The fields of a message its type does not know, kept by a `#[schema(unknown)]` field so a
/// build that reads and writes a value it is older than drops nothing. One pointer when empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unknown {
    /// By id.
    fields: Option<Box<Vec<(u32, Raw)>>>,
}

impl Unknown {
    pub fn is_empty(&self) -> bool {
        self.fields.is_none()
    }

    pub fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.fields
            .iter()
            .flat_map(|fields| fields.iter().map(|(id, _)| *id))
    }

    /// Keeps the field `key` names, which the type does not know: refused when it is marked
    /// must-understand, and dropped when it is marked no-copy.
    pub fn keep(&mut self, key: Key, value: Value<'_>) -> Result<(), Error> {
        let id = crate::understood(key)?;
        if key.no_copy {
            return Ok(());
        }
        let fields = self.fields.get_or_insert_with(Box::default);
        match fields.binary_search_by_key(&id, |(id, _)| *id) {
            Ok(_) => Err(Error::new(ErrorKind::Repeated(id))),
            Err(at) => {
                fields.insert(at, (id, Raw::of(value)?));
                Ok(())
            }
        }
    }

    /// Writes the kept fields from `*next` on whose ids are below `below`, so that, called
    /// before each known field and once with `u32::MAX` at the end, every field goes out in
    /// order of id.
    pub fn write_below(&self, below: u32, next: &mut usize, w: &mut Writer) -> Result<(), Error> {
        let Some(fields) = &self.fields else {
            return Ok(());
        };
        while let Some((id, raw)) = fields.get(*next)
            && *id < below
        {
            raw.write_field(*id, w)?;
            *next += 1;
        }
        Ok(())
    }
}

/// A variant its enum does not know, kept by a `#[schema(unknown)]` case so it can be written
/// back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownVariant {
    pub id: u32,
    raw: Raw,
}

impl UnknownVariant {
    /// Keeps the variant `key` names, unless it is marked must-understand.
    pub fn new(key: Key, value: Value<'_>) -> Result<Self, Error> {
        Ok(UnknownVariant {
            id: crate::understood(key)?,
            raw: Raw::of(value)?,
        })
    }

    pub fn write(&self, w: &mut Writer) -> Result<(), Error> {
        self.raw.write_field(self.id, w)
    }
}

/// A value held as its bytes, decoded when asked for (`get`). Bytes that do not decode (a
/// newer build's, with a must-understand field this one does not know) are kept and written
/// back unchanged, so they fail only whoever asks for the value, not the value holding them.
pub struct Kept<T> {
    raw: Raw,
    _type: PhantomData<fn() -> T>,
}

impl<T: Encode> Kept<T> {
    pub fn new(value: &T) -> Result<Self, Error> {
        let mut w = Writer::new().outermost(T::WIRE);
        value.encode(&mut w)?;
        Ok(Kept {
            raw: Raw {
                wire: T::WIRE,
                contents: w.into_bytes(),
            },
            _type: PhantomData,
        })
    }
}

impl<T: for<'a> Decode<'a>> Kept<T> {
    /// The value, decoded from the bytes kept, with a budget of its own.
    pub fn get(&self) -> Result<T, Error> {
        let budget = BUDGET_BASE
            .saturating_add(BUDGET_PER_BYTE.saturating_mul(self.raw.contents.len() as u64));
        let _budget = Budget::start(budget);
        T::decode(self.raw.value()?)
    }
}

impl<T> Clone for Kept<T> {
    fn clone(&self) -> Self {
        Kept {
            raw: self.raw.clone(),
            _type: PhantomData,
        }
    }
}

impl<T> PartialEq for Kept<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<T> fmt::Debug for Kept<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Kept").field(&self.raw).finish()
    }
}

impl<T: Encode> Encode for Kept<T> {
    const WIRE: Wire = T::WIRE;
    const FROZEN: bool = T::FROZEN;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        w.bytes(&self.raw.contents);
        Ok(())
    }
}

impl<'de, T: Encode> Decode<'de> for Kept<T> {
    fn decode(value: Value<'de>) -> Result<Self, Error> {
        let fits = value.wire() == T::WIRE || (T::WIRE == Wire::Message && value == Value::Empty);
        if !fits {
            return Err(value.mismatch(T::WIRE));
        }
        let mut raw = Raw::of(value)?;
        raw.wire = T::WIRE;
        Ok(Kept {
            raw,
            _type: PhantomData,
        })
    }
}
