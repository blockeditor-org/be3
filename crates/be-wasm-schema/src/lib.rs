//! One encoding for everything be3 saves or sends, defined once, by the Rust type.
//!
//! A prototype: nothing uses it yet (plans/unified_serialization.md says what would).
//!
//! `#[derive(Schema)]` on a struct or enum implements `Encode` and `Decode`. Each field and
//! variant is written with an id from the crate's `schema.lock` (be-wasm-schema-lock), which
//! ./scripts/verify keeps: the source may reorder members freely, and a removed member's id is
//! never given again. A file that calls `be_wasm_schema::frozen!()` freezes its types: from
//! then on verify (and the derive) refuse what an older build could not read.
//!
//! The encoding is tagged, as protobuf's is: a message is fields, each a key (its id and wire
//! type) and a value that is a varint, 4 or 8 bytes, or a length and that many bytes. So:
//!
//! - **A reader skips what it does not know.** An unknown field is skipped, or kept and
//!   written back by a `#[schema(unknown)] rest: Unknown` field. An enum with a
//!   `#[schema(unknown)] Other(UnknownVariant)` case keeps variants it does not know.
//! - **A missing field decodes as its default** where the type has one (`Decode::absent`):
//!   numbers, strings, collections, `Option`, structs whose fields all have one, and enums
//!   with a `#[schema(default)]` unit variant. A field added to a frozen type must have one.
//! - **Encoding is canonical**: fields by id, a field equal to its default left out, map
//!   entries sorted, NaN as one bit pattern; the same value is always the same bytes, so
//!   encoded values can be hashed and signed.
//! - **Bytes are bytes.** Every list goes through `Encode::encode_list`, which `u8` writes as
//!   one copy, so `Vec<u8>`, `[u8; N]` and `&[u8]` never travel byte by byte.
//! - **Decoding borrows where it can**: `&'de str` and `&'de [u8]` point into the input.

extern crate self as be_wasm_schema;

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::{BuildHasher, Hash};

pub use be_wasm_schema_derive::Schema;

/// Freezes every `Schema` type in the file that calls it (`be_wasm_schema::frozen!();`, at its
/// top level). It expands to nothing: ./scripts/verify reads it, records it in `schema.lock`,
/// and from then on refuses to unfreeze the file's types or change them incompatibly.
#[macro_export]
macro_rules! frozen {
    () => {};
}

/// How a value is laid out, which a reader needs to skip it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    Varint = 0,
    Fixed64 = 1,
    /// A varint length, then that many bytes.
    Len = 2,
    Fixed32 = 5,
}

/// A value as read, before its type decodes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value<'de> {
    Varint(u64),
    Fixed64(u64),
    Len(&'de [u8]),
    Fixed32(u32),
}

impl<'de> Value<'de> {
    pub fn wire(&self) -> Wire {
        match self {
            Value::Varint(_) => Wire::Varint,
            Value::Fixed64(_) => Wire::Fixed64,
            Value::Len(_) => Wire::Len,
            Value::Fixed32(_) => Wire::Fixed32,
        }
    }

    fn mismatch(&self, expected: Wire) -> Error {
        Error::Wire {
            expected,
            found: self.wire(),
        }
    }

    pub fn varint(self) -> Result<u64, Error> {
        match self {
            Value::Varint(value) => Ok(value),
            other => Err(other.mismatch(Wire::Varint)),
        }
    }

    pub fn bytes(self) -> Result<&'de [u8], Error> {
        match self {
            Value::Len(bytes) => Ok(bytes),
            other => Err(other.mismatch(Wire::Len)),
        }
    }

    fn fixed32(self) -> Result<u32, Error> {
        match self {
            Value::Fixed32(value) => Ok(value),
            other => Err(other.mismatch(Wire::Fixed32)),
        }
    }

    fn fixed64(self) -> Result<u64, Error> {
        match self {
            Value::Fixed64(value) => Ok(value),
            other => Err(other.mismatch(Wire::Fixed64)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The input ended inside a value.
    Truncated,
    /// A varint longer than ten bytes or past `u64`.
    Overlong,
    /// A key with a wire type the format does not have, or id zero.
    BadKey(u64),
    Wire {
        expected: Wire,
        found: Wire,
    },
    /// A field with no value to default to was missing: `Type.field`.
    Missing(&'static str),
    /// A field given twice, by id.
    Repeated(u32),
    /// A map with a key given twice.
    RepeatedKey,
    /// An enum with no variant, or more than one.
    NotOneVariant,
    /// A variant this enum does not know and has no `#[schema(unknown)]` case for.
    UnknownVariant(u32),
    /// A number too large for its type, a bool or char that is not one, or an array of the
    /// wrong length.
    OutOfRange,
    Utf8,
    /// Bytes after the value.
    Trailing(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for Error {}

/// A type the format can write.
pub trait Encode {
    const WIRE: Wire;

    /// The value alone: a varint, its fixed bytes, or a `Len` value's contents, whose length
    /// whoever holds it writes.
    fn encode(&self, out: &mut Vec<u8>);

    /// Whether this is what a missing field decodes as, so a field holding it is left out.
    fn is_absent(&self) -> bool {
        false
    }

    /// The value as field `id` of a message: left out when absent.
    fn encode_field(&self, id: u32, out: &mut Vec<u8>) {
        if !self.is_absent() {
            write_field(id, self, out);
        }
    }

    /// A list's contents. Each item in turn, length first where it has one; `u8` writes them
    /// as they are.
    fn encode_list(items: &[Self], out: &mut Vec<u8>)
    where
        Self: Sized,
    {
        for item in items {
            write_value(item, out);
        }
    }
}

/// A type the format can read from bytes that live for `'de`.
pub trait Decode<'de>: Encode + Sized {
    /// Whether a missing field of this type decodes (as `absent`). The derive checks that
    /// every field added to a frozen type is.
    const OPTIONAL: bool = false;

    fn decode(value: Value<'de>) -> Result<Self, Error>;

    /// What a missing field decodes as, if anything.
    fn absent() -> Option<Self> {
        None
    }

    /// Field `name` (`Type.field`, for the error), from its value if it was there.
    fn decode_field(value: Option<Value<'de>>, name: &'static str) -> Result<Self, Error> {
        match value {
            Some(value) => Self::decode(value),
            None => Self::absent().ok_or(Error::Missing(name)),
        }
    }

    /// A list's contents, as `Encode::encode_list` wrote them.
    fn decode_list(bytes: &'de [u8]) -> Result<Vec<Self>, Error> {
        let mut reader = Reader::new(bytes);
        let mut items = Vec::new();
        while !reader.is_empty() {
            items.push(Self::decode(reader.value(Self::WIRE)?)?);
        }
        Ok(items)
    }
}

/// `value` encoded alone, as `decode` reads it.
pub fn encode<T: Encode + ?Sized>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    value.encode(&mut out);
    out
}

/// A `T` from exactly `bytes`, as `encode` wrote it.
pub fn decode<'de, T: Decode<'de>>(bytes: &'de [u8]) -> Result<T, Error> {
    if T::WIRE == Wire::Len {
        return T::decode(Value::Len(bytes));
    }
    let mut reader = Reader::new(bytes);
    let value = reader.value(T::WIRE)?;
    if !reader.is_empty() {
        return Err(Error::Trailing(reader.remaining()));
    }
    T::decode(value)
}

pub fn write_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push(value as u8 | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

pub fn write_key(id: u32, wire: Wire, out: &mut Vec<u8>) {
    write_varint((u64::from(id) << 3) | wire as u64, out);
}

/// Runs `body`, then puts the length of what it wrote in front of it.
pub fn with_length(out: &mut Vec<u8>, body: impl FnOnce(&mut Vec<u8>)) {
    let start = out.len();
    body(out);
    let length = out.len() - start;
    let mut prefix = Vec::with_capacity(10);
    write_varint(length as u64, &mut prefix);
    out.splice(start..start, prefix);
}

/// `value` as a list item or a field's value: with its length first if it has one.
pub fn write_value<T: Encode + ?Sized>(value: &T, out: &mut Vec<u8>) {
    if T::WIRE == Wire::Len {
        with_length(out, |out| value.encode(out));
    } else {
        value.encode(out);
    }
}

/// `value` as field `id`, even when it is absent (an enum's variant, or what `Some` holds).
pub fn write_field<T: Encode + ?Sized>(id: u32, value: &T, out: &mut Vec<u8>) {
    write_key(id, T::WIRE, out);
    write_value(value, out);
}

/// Reads values and fields out of bytes, borrowing from them.
pub struct Reader<'de> {
    bytes: &'de [u8],
    at: usize,
}

impl<'de> Reader<'de> {
    pub fn new(bytes: &'de [u8]) -> Self {
        Reader { bytes, at: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.at == self.bytes.len()
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    pub fn varint(&mut self) -> Result<u64, Error> {
        let mut value = 0u64;
        for shift in (0..70).step_by(7) {
            let byte = *self.bytes.get(self.at).ok_or(Error::Truncated)?;
            self.at += 1;
            let bits = u64::from(byte & 0x7f);
            if shift == 63 && bits > 1 {
                return Err(Error::Overlong);
            }
            value |= bits << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(Error::Overlong)
    }

    fn take(&mut self, length: usize) -> Result<&'de [u8], Error> {
        let end = self.at.checked_add(length).ok_or(Error::Truncated)?;
        let bytes = self.bytes.get(self.at..end).ok_or(Error::Truncated)?;
        self.at = end;
        Ok(bytes)
    }

    pub fn value(&mut self, wire: Wire) -> Result<Value<'de>, Error> {
        Ok(match wire {
            Wire::Varint => Value::Varint(self.varint()?),
            Wire::Fixed64 => Value::Fixed64(u64::from_le_bytes(
                self.take(8)?.try_into().expect("eight bytes"),
            )),
            Wire::Fixed32 => Value::Fixed32(u32::from_le_bytes(
                self.take(4)?.try_into().expect("four bytes"),
            )),
            Wire::Len => {
                let length = usize::try_from(self.varint()?).map_err(|_| Error::Truncated)?;
                Value::Len(self.take(length)?)
            }
        })
    }

    /// The next field of a message: its id and value.
    pub fn field(&mut self) -> Result<(u32, Value<'de>), Error> {
        let key = self.varint()?;
        let wire = match key & 7 {
            0 => Wire::Varint,
            1 => Wire::Fixed64,
            2 => Wire::Len,
            5 => Wire::Fixed32,
            _ => return Err(Error::BadKey(key)),
        };
        let id = u32::try_from(key >> 3).map_err(|_| Error::BadKey(key))?;
        if id == 0 {
            return Err(Error::BadKey(key));
        }
        Ok((id, self.value(wire)?))
    }
}

/// A value kept as it was read, to be written back unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Raw {
    Varint(u64),
    Fixed64(u64),
    Len(Vec<u8>),
    Fixed32(u32),
}

impl Raw {
    fn of(value: Value<'_>) -> Raw {
        match value {
            Value::Varint(value) => Raw::Varint(value),
            Value::Fixed64(value) => Raw::Fixed64(value),
            Value::Len(bytes) => Raw::Len(bytes.to_vec()),
            Value::Fixed32(value) => Raw::Fixed32(value),
        }
    }

    fn write_field(&self, id: u32, out: &mut Vec<u8>) {
        match self {
            Raw::Varint(value) => {
                write_key(id, Wire::Varint, out);
                write_varint(*value, out);
            }
            Raw::Fixed64(value) => {
                write_key(id, Wire::Fixed64, out);
                out.extend_from_slice(&value.to_le_bytes());
            }
            Raw::Len(bytes) => {
                write_key(id, Wire::Len, out);
                write_varint(bytes.len() as u64, out);
                out.extend_from_slice(bytes);
            }
            Raw::Fixed32(value) => {
                write_key(id, Wire::Fixed32, out);
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
}

/// The fields of a message its type does not know, kept by a `#[schema(unknown)]` field so a
/// build that reads and writes a value it is older than drops nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unknown {
    /// By id.
    fields: Vec<(u32, Raw)>,
}

impl Unknown {
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.fields.iter().map(|(id, _)| *id)
    }

    /// Keeps field `id`, which the type does not know.
    pub fn keep(&mut self, id: u32, value: Value<'_>) -> Result<(), Error> {
        match self.fields.binary_search_by_key(&id, |(id, _)| *id) {
            Ok(_) => Err(Error::Repeated(id)),
            Err(at) => {
                self.fields.insert(at, (id, Raw::of(value)));
                Ok(())
            }
        }
    }

    /// Writes the kept fields from `*next` on whose ids are below `below`, so that, called
    /// before each known field and once with `u32::MAX` at the end, every field goes out in
    /// order of id.
    pub fn write_below(&self, below: u32, next: &mut usize, out: &mut Vec<u8>) {
        while let Some((id, raw)) = self.fields.get(*next)
            && *id < below
        {
            raw.write_field(*id, out);
            *next += 1;
        }
    }
}

/// A variant its enum does not know, kept by a `#[schema(unknown)]` case so it can be written
/// back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownVariant {
    pub id: u32,
    value: Raw,
}

impl UnknownVariant {
    pub fn new(id: u32, value: Value<'_>) -> Self {
        UnknownVariant {
            id,
            value: Raw::of(value),
        }
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        self.value.write_field(self.id, out);
    }
}

/// The one field of an enum's message: the variant's id and value.
pub fn variant<'de>(value: Value<'de>) -> Result<(u32, Value<'de>), Error> {
    let mut reader = Reader::new(value.bytes()?);
    if reader.is_empty() {
        return Err(Error::NotOneVariant);
    }
    let field = reader.field()?;
    if !reader.is_empty() {
        return Err(Error::NotOneVariant);
    }
    Ok(field)
}

/// Stores `value` as the field `id` read in a message, refusing it a second time.
pub fn once<'de>(slot: &mut Option<Value<'de>>, id: u32, value: Value<'de>) -> Result<(), Error> {
    match slot.replace(value) {
        Some(_) => Err(Error::Repeated(id)),
        None => Ok(()),
    }
}

macro_rules! unsigned {
    ($($ty:ty),*) => {$(
        impl Encode for $ty {
            const WIRE: Wire = Wire::Varint;
            fn encode(&self, out: &mut Vec<u8>) {
                write_varint(*self as u64, out);
            }
            fn is_absent(&self) -> bool {
                *self == 0
            }
        }
        impl<'de> Decode<'de> for $ty {
            const OPTIONAL: bool = true;
            fn decode(value: Value<'de>) -> Result<Self, Error> {
                <$ty>::try_from(value.varint()?).map_err(|_| Error::OutOfRange)
            }
            fn absent() -> Option<Self> {
                Some(0)
            }
        }
    )*};
}

unsigned!(u16, u32, u64, usize);

impl Encode for u8 {
    const WIRE: Wire = Wire::Varint;

    fn encode(&self, out: &mut Vec<u8>) {
        write_varint(u64::from(*self), out);
    }

    fn is_absent(&self) -> bool {
        *self == 0
    }

    fn encode_list(items: &[Self], out: &mut Vec<u8>) {
        out.extend_from_slice(items);
    }
}

impl<'de> Decode<'de> for u8 {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        u8::try_from(value.varint()?).map_err(|_| Error::OutOfRange)
    }

    fn absent() -> Option<Self> {
        Some(0)
    }

    fn decode_list(bytes: &'de [u8]) -> Result<Vec<Self>, Error> {
        Ok(bytes.to_vec())
    }
}

macro_rules! signed {
    ($($ty:ty),*) => {$(
        impl Encode for $ty {
            const WIRE: Wire = Wire::Varint;
            fn encode(&self, out: &mut Vec<u8>) {
                let value = *self as i64;
                write_varint(((value << 1) ^ (value >> 63)) as u64, out);
            }
            fn is_absent(&self) -> bool {
                *self == 0
            }
        }
        impl<'de> Decode<'de> for $ty {
            const OPTIONAL: bool = true;
            fn decode(value: Value<'de>) -> Result<Self, Error> {
                let raw = value.varint()?;
                let value = ((raw >> 1) as i64) ^ -((raw & 1) as i64);
                <$ty>::try_from(value).map_err(|_| Error::OutOfRange)
            }
            fn absent() -> Option<Self> {
                Some(0)
            }
        }
    )*};
}

signed!(i8, i16, i32, i64, isize);

impl Encode for bool {
    const WIRE: Wire = Wire::Varint;

    fn encode(&self, out: &mut Vec<u8>) {
        out.push(u8::from(*self));
    }

    fn is_absent(&self) -> bool {
        !*self
    }
}

impl<'de> Decode<'de> for bool {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        match value.varint()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::OutOfRange),
        }
    }

    fn absent() -> Option<Self> {
        Some(false)
    }
}

impl Encode for char {
    const WIRE: Wire = Wire::Varint;

    fn encode(&self, out: &mut Vec<u8>) {
        write_varint(u64::from(u32::from(*self)), out);
    }

    fn is_absent(&self) -> bool {
        *self == '\0'
    }
}

impl<'de> Decode<'de> for char {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        u32::try_from(value.varint()?)
            .ok()
            .and_then(char::from_u32)
            .ok_or(Error::OutOfRange)
    }

    fn absent() -> Option<Self> {
        Some('\0')
    }
}

impl Encode for f32 {
    const WIRE: Wire = Wire::Fixed32;

    fn encode(&self, out: &mut Vec<u8>) {
        let value = if self.is_nan() { f32::NAN } else { *self };
        out.extend_from_slice(&value.to_bits().to_le_bytes());
    }

    fn is_absent(&self) -> bool {
        self.to_bits() == 0
    }
}

impl<'de> Decode<'de> for f32 {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        Ok(f32::from_bits(value.fixed32()?))
    }

    fn absent() -> Option<Self> {
        Some(0.0)
    }
}

impl Encode for f64 {
    const WIRE: Wire = Wire::Fixed64;

    fn encode(&self, out: &mut Vec<u8>) {
        let value = if self.is_nan() { f64::NAN } else { *self };
        out.extend_from_slice(&value.to_bits().to_le_bytes());
    }

    fn is_absent(&self) -> bool {
        self.to_bits() == 0
    }
}

impl<'de> Decode<'de> for f64 {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        Ok(f64::from_bits(value.fixed64()?))
    }

    fn absent() -> Option<Self> {
        Some(0.0)
    }
}

impl Encode for str {
    const WIRE: Wire = Wire::Len;

    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self.as_bytes());
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl Encode for String {
    const WIRE: Wire = Wire::Len;

    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self.as_bytes());
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<'de> Decode<'de> for String {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        <&str>::decode(value).map(str::to_owned)
    }

    fn absent() -> Option<Self> {
        Some(String::new())
    }
}

impl Encode for &str {
    const WIRE: Wire = Wire::Len;

    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self.as_bytes());
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<'de> Decode<'de> for &'de str {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        std::str::from_utf8(value.bytes()?).map_err(|_| Error::Utf8)
    }

    fn absent() -> Option<Self> {
        Some("")
    }
}

impl Encode for Cow<'_, str> {
    const WIRE: Wire = Wire::Len;

    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self.as_bytes());
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<'de> Decode<'de> for Cow<'de, str> {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        <&str>::decode(value).map(Cow::Borrowed)
    }

    fn absent() -> Option<Self> {
        Some(Cow::Borrowed(""))
    }
}

impl<T: Encode> Encode for [T] {
    const WIRE: Wire = Wire::Len;

    fn encode(&self, out: &mut Vec<u8>) {
        T::encode_list(self, out);
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<T: Encode> Encode for Vec<T> {
    const WIRE: Wire = Wire::Len;

    fn encode(&self, out: &mut Vec<u8>) {
        T::encode_list(self, out);
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<'de, T: Decode<'de>> Decode<'de> for Vec<T> {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        T::decode_list(value.bytes()?)
    }

    fn absent() -> Option<Self> {
        Some(Vec::new())
    }
}

impl Encode for &[u8] {
    const WIRE: Wire = Wire::Len;

    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self);
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<'de> Decode<'de> for &'de [u8] {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        value.bytes()
    }

    fn absent() -> Option<Self> {
        Some(&[])
    }
}

impl<T: Encode, const N: usize> Encode for [T; N] {
    const WIRE: Wire = Wire::Len;

    fn encode(&self, out: &mut Vec<u8>) {
        T::encode_list(self, out);
    }
}

impl<'de, T: Decode<'de>, const N: usize> Decode<'de> for [T; N] {
    fn decode(value: Value<'de>) -> Result<Self, Error> {
        T::decode_list(value.bytes()?)?
            .try_into()
            .map_err(|_| Error::OutOfRange)
    }
}

impl<T: Encode + ?Sized> Encode for Box<T> {
    const WIRE: Wire = T::WIRE;

    fn encode(&self, out: &mut Vec<u8>) {
        (**self).encode(out);
    }

    fn is_absent(&self) -> bool {
        (**self).is_absent()
    }

    fn encode_field(&self, id: u32, out: &mut Vec<u8>) {
        (**self).encode_field(id, out);
    }
}

impl<'de, T: Decode<'de>> Decode<'de> for Box<T> {
    const OPTIONAL: bool = T::OPTIONAL;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        T::decode(value).map(Box::new)
    }

    fn absent() -> Option<Self> {
        T::absent().map(Box::new)
    }

    fn decode_field(value: Option<Value<'de>>, name: &'static str) -> Result<Self, Error> {
        T::decode_field(value, name).map(Box::new)
    }
}

/// As a field, `None` is the field left out and `Some` the field written, even when what it
/// holds is that type's default. Alone or in a list, a message whose field 1 is there or not.
impl<T: Encode> Encode for Option<T> {
    const WIRE: Wire = Wire::Len;

    fn encode(&self, out: &mut Vec<u8>) {
        if let Some(value) = self {
            write_field(1, value, out);
        }
    }

    fn is_absent(&self) -> bool {
        self.is_none()
    }

    fn encode_field(&self, id: u32, out: &mut Vec<u8>) {
        if let Some(value) = self {
            write_field(id, value, out);
        }
    }
}

impl<'de, T: Decode<'de>> Decode<'de> for Option<T> {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        let mut reader = Reader::new(value.bytes()?);
        let mut held = None;
        while !reader.is_empty() {
            let (id, value) = reader.field()?;
            if id == 1 {
                once(&mut held, id, value)?;
            }
        }
        held.map(T::decode).transpose()
    }

    fn absent() -> Option<Self> {
        Some(None)
    }

    fn decode_field(value: Option<Value<'de>>, _: &'static str) -> Result<Self, Error> {
        value.map(T::decode).transpose()
    }
}

fn write_entry<K: Encode, V: Encode>(key: &K, value: &V, out: &mut Vec<u8>) {
    with_length(out, |out| {
        key.encode_field(1, out);
        value.encode_field(2, out);
    });
}

fn read_entries<'de, K: Decode<'de>, V: Decode<'de>>(
    value: Value<'de>,
    mut insert: impl FnMut(K, V) -> bool,
) -> Result<(), Error> {
    let mut reader = Reader::new(value.bytes()?);
    while !reader.is_empty() {
        let mut entry = Reader::new(reader.value(Wire::Len)?.bytes()?);
        let (mut key, mut value) = (None, None);
        while !entry.is_empty() {
            match entry.field()? {
                (1, found) => once(&mut key, 1, found)?,
                (2, found) => once(&mut value, 2, found)?,
                _ => {}
            }
        }
        let key = K::decode_field(key, "map key")?;
        let value = V::decode_field(value, "map value")?;
        if !insert(key, value) {
            return Err(Error::RepeatedKey);
        }
    }
    Ok(())
}

/// A list of entries, each a message of its key (1) and value (2), in the key's order.
impl<K: Encode, V: Encode> Encode for BTreeMap<K, V> {
    const WIRE: Wire = Wire::Len;

    fn encode(&self, out: &mut Vec<u8>) {
        for (key, value) in self {
            write_entry(key, value, out);
        }
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<'de, K: Decode<'de> + Ord, V: Decode<'de>> Decode<'de> for BTreeMap<K, V> {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        let mut map = BTreeMap::new();
        read_entries(value, |key, value| map.insert(key, value).is_none())?;
        Ok(map)
    }

    fn absent() -> Option<Self> {
        Some(BTreeMap::new())
    }
}

/// As `BTreeMap`, with the entries sorted by their encoding so the bytes do not depend on
/// the order the map was filled in.
impl<K: Encode, V: Encode, S> Encode for HashMap<K, V, S> {
    const WIRE: Wire = Wire::Len;

    fn encode(&self, out: &mut Vec<u8>) {
        let mut entries: Vec<Vec<u8>> = self
            .iter()
            .map(|(key, value)| {
                let mut entry = Vec::new();
                write_entry(key, value, &mut entry);
                entry
            })
            .collect();
        entries.sort_unstable();
        for entry in entries {
            out.extend_from_slice(&entry);
        }
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<'de, K: Decode<'de> + Eq + Hash, V: Decode<'de>, S: BuildHasher + Default> Decode<'de>
    for HashMap<K, V, S>
{
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        let mut map = HashMap::default();
        read_entries(value, |key, value| map.insert(key, value).is_none())?;
        Ok(map)
    }

    fn absent() -> Option<Self> {
        Some(HashMap::default())
    }
}

#[cfg(test)]
mod frozen_example;
#[cfg(test)]
mod tests;
