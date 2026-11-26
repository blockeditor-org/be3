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
//! The encoding is tagged, as protobuf's is, and made to decode fast rather than to be small
//! (compression, a layer of its own, wins size back: plans/deferred/compression.md). A message
//! is fields, each a key (a varint of its id and wire type) and a value: 1, 2, 4 or 8 bytes
//! little-endian, or a 4-byte little-endian length and that many bytes. Lengths are fixed-width
//! so the encoder writes every byte where it ends up, never moving what it wrote. So:
//!
//! - **Numbers are fixed-width.** A `u64` is 8 bytes, a `u16` 2, signed ones two's complement.
//!   A list of numbers is their memory, little-endian, so on a little-endian machine (every
//!   one be3 runs on) it encodes and decodes as one copy; `Vec<u8>`, `[u8; N]` and `&[u8]` too.
//! - **A reader skips what it does not know.** An unknown field is skipped, or kept and
//!   written back by a `#[schema(unknown)] rest: Unknown` field. An enum with a
//!   `#[schema(unknown)] Other(UnknownVariant)` case keeps variants it does not know.
//! - **A missing field decodes as its default** where the type has one (`Decode::absent`):
//!   numbers, strings, collections, `Option`, structs whose fields all have one, and enums
//!   with a `#[schema(default)]` unit variant. A field added to a frozen type must have one.
//! - **Encoding is canonical**: fields by id, a field equal to its default left out, map
//!   entries sorted, NaN as one bit pattern; the same value is always the same bytes, so
//!   encoded values can be hashed and signed.
//! - **Decoding borrows where it can**: `&'de str` and `&'de [u8]` point into the input.
//! - **Untrusted input is safe to decode**: values nest at most `MAX_DEPTH` deep, so a
//!   recursive type cannot be made to overflow the stack, and no length is trusted before the
//!   bytes it claims are there.
//!
//! `usize` and `isize` are not encoded (the derive refuses them): their width differs between
//! wasm32 and 64-bit hosts.

extern crate self as be_wasm_schema;

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::{BuildHasher, Hash};

pub use be_wasm_schema_derive::Schema;

#[cfg(any(test, feature = "fuzzing"))]
pub mod fuzz;

/// Freezes every `Schema` type in the file that calls it (`be_wasm_schema::frozen!();`, at its
/// top level). It expands to nothing: ./scripts/verify reads it, records it in `schema.lock`,
/// and from then on refuses to unfreeze the file's types or change them incompatibly.
#[macro_export]
macro_rules! frozen {
    () => {};
}

/// How deep values may nest: a message, list, map, enum or `Option` holding another counts a
/// level. Deeper input is refused, as `Error::TooDeep`.
pub const MAX_DEPTH: u32 = 128;

/// How a value is laid out, which a reader needs to skip it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    Fixed8 = 0,
    Fixed16 = 1,
    Fixed32 = 2,
    Fixed64 = 3,
    /// A 4-byte little-endian length, then that many bytes.
    Len = 4,
}

/// A length-prefixed value's bytes, and how many levels may still nest below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nested<'de> {
    pub bytes: &'de [u8],
    depth: u32,
}

/// A value as read, before its type decodes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value<'de> {
    Fixed8(u8),
    Fixed16(u16),
    Fixed32(u32),
    Fixed64(u64),
    Len(Nested<'de>),
}

impl<'de> Value<'de> {
    pub fn wire(&self) -> Wire {
        match self {
            Value::Fixed8(_) => Wire::Fixed8,
            Value::Fixed16(_) => Wire::Fixed16,
            Value::Fixed32(_) => Wire::Fixed32,
            Value::Fixed64(_) => Wire::Fixed64,
            Value::Len(_) => Wire::Len,
        }
    }

    fn mismatch(&self, expected: Wire) -> Error {
        Error::Wire {
            expected,
            found: self.wire(),
        }
    }

    pub fn fixed8(self) -> Result<u8, Error> {
        match self {
            Value::Fixed8(value) => Ok(value),
            other => Err(other.mismatch(Wire::Fixed8)),
        }
    }

    pub fn fixed16(self) -> Result<u16, Error> {
        match self {
            Value::Fixed16(value) => Ok(value),
            other => Err(other.mismatch(Wire::Fixed16)),
        }
    }

    pub fn fixed32(self) -> Result<u32, Error> {
        match self {
            Value::Fixed32(value) => Ok(value),
            other => Err(other.mismatch(Wire::Fixed32)),
        }
    }

    pub fn fixed64(self) -> Result<u64, Error> {
        match self {
            Value::Fixed64(value) => Ok(value),
            other => Err(other.mismatch(Wire::Fixed64)),
        }
    }

    /// A length-prefixed value's bytes, for a value that holds no others (a string, bytes, a
    /// list of numbers).
    pub fn bytes(self) -> Result<&'de [u8], Error> {
        match self {
            Value::Len(nested) => Ok(nested.bytes),
            other => Err(other.mismatch(Wire::Len)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The input ended inside a value, or a list of numbers ended inside one.
    Truncated,
    /// A key longer than its varint can be, with a wire type the format does not have, or
    /// with id zero.
    BadKey,
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
    /// A bool or char that is not one, or an array of the wrong length.
    OutOfRange,
    Utf8,
    /// Values nested deeper than `MAX_DEPTH`.
    TooDeep,
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

    /// The value alone: its fixed bytes, or a `Len` value's contents, whose length whoever
    /// holds it writes.
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

    /// A list's contents: each item in turn, length first where it has one. Numbers write
    /// their memory as it is (little-endian).
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

    /// A list, as `Encode::encode_list` wrote its contents.
    fn decode_list(value: Value<'de>) -> Result<Vec<Self>, Error> {
        let mut reader = Reader::of(value)?;
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
        return T::decode(Value::Len(Nested {
            bytes,
            depth: MAX_DEPTH,
        }));
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

/// Writes a length, runs `body`, then sets the length to what it wrote.
///
/// Panics if `body` writes 4 GiB or more, which no single value may be.
pub fn with_length(out: &mut Vec<u8>, body: impl FnOnce(&mut Vec<u8>)) {
    let at = out.len();
    out.extend_from_slice(&[0; 4]);
    body(out);
    let length = u32::try_from(out.len() - at - 4).expect("a value is smaller than 4 GiB");
    out[at..at + 4].copy_from_slice(&length.to_le_bytes());
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

/// Field `id` holding nothing: a unit variant.
pub fn write_empty(id: u32, out: &mut Vec<u8>) {
    write_key(id, Wire::Len, out);
    out.extend_from_slice(&[0; 4]);
}

/// Reads values and fields out of a message or list, borrowing from it.
pub struct Reader<'de> {
    bytes: &'de [u8],
    at: usize,
    /// How many levels the values it reads may still nest.
    depth: u32,
}

impl<'de> Reader<'de> {
    /// Reads `bytes` as the outermost value.
    pub fn new(bytes: &'de [u8]) -> Self {
        Reader {
            bytes,
            at: 0,
            depth: MAX_DEPTH,
        }
    }

    /// Reads inside `value`, a length-prefixed value holding others: a level deeper.
    pub fn of(value: Value<'de>) -> Result<Self, Error> {
        let Value::Len(nested) = value else {
            return Err(value.mismatch(Wire::Len));
        };
        Ok(Reader {
            bytes: nested.bytes,
            at: 0,
            depth: nested.depth.checked_sub(1).ok_or(Error::TooDeep)?,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.at == self.bytes.len()
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    fn take(&mut self, length: usize) -> Result<&'de [u8], Error> {
        let end = self.at.checked_add(length).ok_or(Error::Truncated)?;
        let bytes = self.bytes.get(self.at..end).ok_or(Error::Truncated)?;
        self.at = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        Ok(self.take(N)?.try_into().expect("N bytes"))
    }

    fn key(&mut self) -> Result<u64, Error> {
        let mut key = 0u64;
        for shift in (0..35).step_by(7) {
            let byte = *self.bytes.get(self.at).ok_or(Error::Truncated)?;
            self.at += 1;
            key |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(key);
            }
        }
        Err(Error::BadKey)
    }

    pub fn value(&mut self, wire: Wire) -> Result<Value<'de>, Error> {
        Ok(match wire {
            Wire::Fixed8 => Value::Fixed8(self.array::<1>()?[0]),
            Wire::Fixed16 => Value::Fixed16(u16::from_le_bytes(self.array()?)),
            Wire::Fixed32 => Value::Fixed32(u32::from_le_bytes(self.array()?)),
            Wire::Fixed64 => Value::Fixed64(u64::from_le_bytes(self.array()?)),
            Wire::Len => {
                let length = u32::from_le_bytes(self.array()?) as usize;
                Value::Len(Nested {
                    bytes: self.take(length)?,
                    depth: self.depth,
                })
            }
        })
    }

    /// The next field of a message: its id and value.
    pub fn field(&mut self) -> Result<(u32, Value<'de>), Error> {
        let key = self.key()?;
        let wire = match key & 7 {
            0 => Wire::Fixed8,
            1 => Wire::Fixed16,
            2 => Wire::Fixed32,
            3 => Wire::Fixed64,
            4 => Wire::Len,
            _ => return Err(Error::BadKey),
        };
        let id = u32::try_from(key >> 3).map_err(|_| Error::BadKey)?;
        if id == 0 {
            return Err(Error::BadKey);
        }
        Ok((id, self.value(wire)?))
    }
}

/// A value kept as it was read, to be written back unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Raw {
    Fixed8(u8),
    Fixed16(u16),
    Fixed32(u32),
    Fixed64(u64),
    Len(Vec<u8>),
}

impl Raw {
    fn of(value: Value<'_>) -> Raw {
        match value {
            Value::Fixed8(value) => Raw::Fixed8(value),
            Value::Fixed16(value) => Raw::Fixed16(value),
            Value::Fixed32(value) => Raw::Fixed32(value),
            Value::Fixed64(value) => Raw::Fixed64(value),
            Value::Len(nested) => Raw::Len(nested.bytes.to_vec()),
        }
    }

    fn write_field(&self, id: u32, out: &mut Vec<u8>) {
        match self {
            Raw::Fixed8(value) => write_field(id, value, out),
            Raw::Fixed16(value) => write_field(id, value, out),
            Raw::Fixed32(value) => write_field(id, value, out),
            Raw::Fixed64(value) => write_field(id, value, out),
            Raw::Len(bytes) => write_field(id, bytes.as_slice(), out),
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
pub fn variant(value: Value<'_>) -> Result<(u32, Value<'_>), Error> {
    let mut reader = Reader::of(value)?;
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

/// A list of numbers' bytes, which are their memory on a little-endian machine.
fn number_bytes<T: bytemuck::NoUninit>(items: &[T], out: &mut Vec<u8>, swap: impl Fn(&T) -> T) {
    if cfg!(target_endian = "little") {
        out.extend_from_slice(bytemuck::cast_slice(items));
    } else {
        for item in items {
            out.extend_from_slice(bytemuck::bytes_of(&swap(item)));
        }
    }
}

/// Numbers from their little-endian bytes: one copy on a little-endian machine.
fn numbers_from<T: bytemuck::Pod>(bytes: &[u8], swap: impl Fn(T) -> T) -> Result<Vec<T>, Error> {
    let size = size_of::<T>();
    if bytes.len() % size != 0 {
        return Err(Error::Truncated);
    }
    let mut items = vec![T::zeroed(); bytes.len() / size];
    bytemuck::cast_slice_mut(&mut items).copy_from_slice(bytes);
    if cfg!(target_endian = "big") {
        for item in &mut items {
            *item = swap(*item);
        }
    }
    Ok(items)
}

macro_rules! integers {
    ($($ty:ty: $wire:ident, $read:ident;)*) => {$(
        impl Encode for $ty {
            const WIRE: Wire = Wire::$wire;
            fn encode(&self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_le_bytes());
            }
            fn is_absent(&self) -> bool {
                *self == 0
            }
            fn encode_list(items: &[Self], out: &mut Vec<u8>) {
                number_bytes(items, out, |item| item.swap_bytes());
            }
        }
        impl<'de> Decode<'de> for $ty {
            const OPTIONAL: bool = true;
            fn decode(value: Value<'de>) -> Result<Self, Error> {
                Ok(<$ty>::from_le_bytes(value.$read()?.to_le_bytes()))
            }
            fn absent() -> Option<Self> {
                Some(0)
            }
            fn decode_list(value: Value<'de>) -> Result<Vec<Self>, Error> {
                numbers_from(value.bytes()?, <$ty>::swap_bytes)
            }
        }
    )*};
}

integers! {
    u8: Fixed8, fixed8;
    i8: Fixed8, fixed8;
    u16: Fixed16, fixed16;
    i16: Fixed16, fixed16;
    u32: Fixed32, fixed32;
    i32: Fixed32, fixed32;
    u64: Fixed64, fixed64;
    i64: Fixed64, fixed64;
}

macro_rules! floats {
    ($($ty:ty: $wire:ident, $read:ident;)*) => {$(
        /// Every NaN is written as `NAN`, so equal values are equal bytes.
        impl Encode for $ty {
            const WIRE: Wire = Wire::$wire;
            fn encode(&self, out: &mut Vec<u8>) {
                let value = if self.is_nan() { <$ty>::NAN } else { *self };
                out.extend_from_slice(&value.to_bits().to_le_bytes());
            }
            fn is_absent(&self) -> bool {
                self.to_bits() == 0
            }
            fn encode_list(items: &[Self], out: &mut Vec<u8>) {
                if items.iter().any(|item| item.is_nan()) {
                    for item in items {
                        item.encode(out);
                    }
                } else {
                    number_bytes(items, out, |item| <$ty>::from_bits(item.to_bits().swap_bytes()));
                }
            }
        }
        impl<'de> Decode<'de> for $ty {
            const OPTIONAL: bool = true;
            fn decode(value: Value<'de>) -> Result<Self, Error> {
                Ok(<$ty>::from_bits(value.$read()?))
            }
            fn absent() -> Option<Self> {
                Some(0.0)
            }
            fn decode_list(value: Value<'de>) -> Result<Vec<Self>, Error> {
                numbers_from(value.bytes()?, |item: $ty| <$ty>::from_bits(item.to_bits().swap_bytes()))
            }
        }
    )*};
}

floats! {
    f32: Fixed32, fixed32;
    f64: Fixed64, fixed64;
}

impl Encode for bool {
    const WIRE: Wire = Wire::Fixed8;

    fn encode(&self, out: &mut Vec<u8>) {
        out.push(u8::from(*self));
    }

    fn is_absent(&self) -> bool {
        !*self
    }

    fn encode_list(items: &[Self], out: &mut Vec<u8>) {
        out.extend_from_slice(bytemuck::cast_slice(items));
    }
}

impl<'de> Decode<'de> for bool {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        match value.fixed8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::OutOfRange),
        }
    }

    fn absent() -> Option<Self> {
        Some(false)
    }

    fn decode_list(value: Value<'de>) -> Result<Vec<Self>, Error> {
        value
            .bytes()?
            .iter()
            .map(|byte| Self::decode(Value::Fixed8(*byte)))
            .collect()
    }
}

impl Encode for char {
    const WIRE: Wire = Wire::Fixed32;

    fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&u32::from(*self).to_le_bytes());
    }

    fn is_absent(&self) -> bool {
        *self == '\0'
    }
}

impl<'de> Decode<'de> for char {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        char::from_u32(value.fixed32()?).ok_or(Error::OutOfRange)
    }

    fn absent() -> Option<Self> {
        Some('\0')
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
        T::decode_list(value)
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
        T::decode_list(value)?
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
        let mut reader = Reader::of(value)?;
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
    let mut reader = Reader::of(value)?;
    while !reader.is_empty() {
        let mut entry = Reader::of(reader.value(Wire::Len)?)?;
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
