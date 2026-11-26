//! One encoding for everything be3 saves or sends, defined once, by the Rust type.
//!
//! A prototype: nothing uses it yet (plans/unified_serialization.md says what would).
//!
//! `#[derive(Schema)]` on a struct or enum implements `Encode` and `Decode`. Every type says
//! whether it is `#[schema(frozen)]` (no breaking changes: older builds must keep reading what
//! newer ones write) or `#[schema(unfrozen)]` (change it however it needs), and a frozen type
//! may hold only frozen types, which the compiler checks through `Encode::FROZEN`. Each field and
//! variant is written with an id from the crate's `schema.lock` (be-wasm-schema-lock), which
//! ./scripts/verify keeps: the source may reorder members freely, and a removed member's id is
//! never given again; for a frozen type, verify (and the derive) refuse what an older build could
//! not read.
//!
//! The encoding is tagged, as protobuf's is, and made to decode fast rather than to be small
//! (compression, a layer of its own, wins size back: plans/deferred/compression.md). A message
//! is fields, each a key and a value. The key is a varint of
//! `id << 6 | no_copy << 5 | critical << 4 | wire`, and the wire type (`Wire`) says how to read or
//! skip the value: nothing, 1, 2, 4, 8 or 16 bytes little-endian, a block reference, or a 4-byte
//! little-endian length and that many bytes of bytes, a message, a list or packed numbers. Six of
//! the sixteen wire types are spare. Lengths are fixed-width so the encoder writes every byte
//! where it ends up. So:
//!
//! - **Numbers are fixed-width.** A `u64` is 8 bytes, a `u16` 2, signed ones two's complement;
//!   a number decodes from any narrower width of its signedness, so widening a field reads old
//!   data. A list of numbers is packed: a byte naming the element type, then their memory,
//!   little-endian, so on a little-endian machine (every one be3 runs on) it encodes and decodes
//!   as one copy. `Vec<u8>` and the like are bytes.
//! - **Unknown data can be walked without the schema**: a message, a list and bytes have wire
//!   types of their own, so `references` finds every block reference (`Reference`) in a value a
//!   newer build wrote, and `dump` prints any value.
//! - **A reader skips what it does not know.** An unknown field is skipped, or kept and
//!   written back by a `#[schema(unknown)] rest: Unknown` field. An enum with a
//!   `#[schema(unknown)] Other(UnknownVariant)` case keeps variants it does not know.
//! - **Changes older builds must not misread are marked so**, in the key. A `#[schema(critical)]`
//!   member is one a reader that does not know it refuses (`ErrorKind::NotUnderstood`) rather
//!   than skipping. A `#[schema(no_copy)]` field is one such a reader may read past but never
//!   writes back (it is dropped, not kept), for a field whose meaning depends on others.
//! - **A missing field decodes as its default** where the type has one (`Decode::absent`), or as
//!   the field's own `#[schema(default = ...)]`. A field added to a frozen type must have one.
//! - **`Kept<T>`** holds a value's bytes and decodes them on demand, keeping them when they do
//!   not decode, so one value an older build cannot read does not fail the whole.
//! - **Encoding is canonical**: fields by id, a field equal to its default left out, an empty
//!   message written as nothing, map entries and set elements sorted by their encoded bytes, NaN
//!   as one bit pattern; the same value is always the same bytes, so encoded values can be
//!   hashed and signed, and `decode_canonical` refuses any other bytes.
//! - **Decoding borrows where it can**: `&'de str` and `&'de [u8]` point into the input.
//! - **Untrusted input is safe to decode**: values nest at most `MAX_DEPTH` deep, in encoding as
//!   in decoding; no length is trusted before the bytes it claims are there; and what decoding
//!   allocates is held to a budget proportional to the input (`decode_with_budget`).
//! - **Errors say where**: an error carries the path of fields it was found under.
//!
//! `usize` and `isize` are not encoded (the derive refuses them): their width differs between
//! wasm32 and 64-bit hosts.

extern crate self as be_wasm_schema;

mod impls;
mod kept;
mod walk;
mod wire;
mod writer;

use std::cell::Cell;
use std::fmt;

pub use be_wasm_schema_derive::Schema;
pub use kept::{Kept, Raw, Unknown, UnknownVariant};
pub use walk::{dump, references};
pub use wire::{Items, ItemsReader, Key, Nested, Number, Packed, Reader, Reference, Value, Wire};
pub use writer::Writer;

#[cfg(any(test, feature = "fuzzing"))]
pub mod fuzz;

/// How deep values may nest: a message or a list holding another counts a level, in encoding as
/// in decoding. Deeper is refused, as `ErrorKind::TooDeep`. Fixed, since data nested deeper
/// than one build allows would be unreadable by it.
pub const MAX_DEPTH: u32 = 128;

/// What a decode may allocate when no budget is given: this much, plus `BUDGET_PER_BYTE` for
/// each byte of input.
pub const BUDGET_BASE: u64 = 1 << 20;
pub const BUDGET_PER_BYTE: u64 = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// The input ended inside a value, or packed numbers ended inside one.
    Truncated,
    /// A key longer than its varint can be, with a wire type the format does not have, or with
    /// id zero.
    BadKey,
    Wire {
        expected: Wire,
        found: Wire,
    },
    /// Packed numbers of another type than the list holds, and not a narrower one of its kind.
    Number {
        expected: Number,
        found: Number,
    },
    /// A list whose items are not the type it holds, or whose count is not what it holds.
    BadList,
    /// A field with no value to default to was missing.
    Missing,
    /// A field given twice, by id.
    Repeated(u32),
    /// A map with a key given twice, or a set with an element given twice.
    RepeatedKey,
    /// An enum with no variant, or more than one.
    NotOneVariant,
    /// A variant this enum does not know and has no `#[schema(unknown)]` case for.
    UnknownVariant(u32),
    /// A field or variant marked must-understand that this type does not know, by id: data
    /// from a newer build that changed in a way this one cannot read.
    NotUnderstood(u32),
    /// A bool, char or nonzero number that is not one, a duration's nanoseconds past a second,
    /// or an array of the wrong length.
    OutOfRange,
    Utf8,
    /// Values nested deeper than `MAX_DEPTH`.
    TooDeep,
    /// A value of 4 GiB or more, which no length can say.
    TooLarge,
    /// Decoding would allocate more than its budget.
    OverBudget,
    /// Bytes `decode_canonical` refused: not what encoding the value gives.
    NotCanonical,
    /// Bytes after the value.
    Trailing(usize),
}

/// What went wrong, and under which fields (`Type.field`, outermost first).
#[derive(Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    /// Innermost first, as they are added while the error leaves each field.
    path: Vec<&'static str>,
}

impl Error {
    pub fn new(kind: ErrorKind) -> Self {
        Error {
            kind,
            path: Vec::new(),
        }
    }

    /// The error, found under field `name`.
    pub fn within(mut self, name: &'static str) -> Self {
        self.path.push(name);
        self
    }

    /// The fields it was found under, outermost first.
    pub fn path(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.path.iter().rev().copied()
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Error::new(kind)
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, name) in self.path().enumerate() {
            if index > 0 {
                f.write_str(" > ")?;
            }
            f.write_str(name)?;
        }
        if !self.path.is_empty() {
            f.write_str(": ")?;
        }
        write!(f, "{:?}", self.kind)
    }
}

impl std::error::Error for Error {}

/// How a type's values sit in a list of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListForm {
    /// A list (`Wire::List`) of items of the type's wire type.
    Items,
    /// Bytes (`Wire::Bytes`): `u8`.
    Bytes,
    /// Packed numbers (`Wire::Packed`) of this type.
    Packed(Number),
}

impl ListForm {
    pub const fn wire(self) -> Wire {
        match self {
            ListForm::Items => Wire::List,
            ListForm::Bytes => Wire::Bytes,
            ListForm::Packed(_) => Wire::Packed,
        }
    }
}

/// A type the format can write.
pub trait Encode {
    const WIRE: Wire;

    /// Whether the type is frozen: whether every later build must read what this one writes.
    /// A frozen type may hold only frozen types (the derive checks it at compile time).
    const FROZEN: bool;

    /// How its values sit in a list.
    const LIST: ListForm = ListForm::Items;

    /// What the derive checks at compile time, forced by `encode`: that a frozen type holds only
    /// frozen types.
    #[doc(hidden)]
    const SCHEMA_CHECKS: () = ();

    /// The value alone: its fixed bytes, or a length-prefixed value's contents, whose length
    /// whoever holds it writes.
    fn encode(&self, w: &mut Writer) -> Result<(), Error>;

    /// Whether this is what a missing field decodes as, so a field holding it is left out.
    fn is_absent(&self) -> bool {
        false
    }

    /// The value as the field `key` names: left out when absent.
    fn encode_field(&self, key: Key, w: &mut Writer) -> Result<(), Error> {
        if self.is_absent() {
            Ok(())
        } else {
            w.field(key, self)
        }
    }

    /// A list's contents, as `Self::LIST` says: by default a list of items, each length first
    /// where it has one.
    fn encode_list(items: &[Self], w: &mut Writer) -> Result<(), Error>
    where
        Self: Sized,
    {
        w.list_header(Self::WIRE, items.len())?;
        for item in items {
            w.value(item)?;
        }
        Ok(())
    }
}

/// A type the format can read from bytes that live for `'de`.
pub trait Decode<'de>: Encode + Sized {
    /// Whether a missing field of this type decodes (as `absent`). The derive checks that
    /// every field added to a frozen type does.
    const OPTIONAL: bool = false;

    /// What the derive checks at compile time, forced by `decode`: that a field added to a frozen
    /// type decodes when missing.
    #[doc(hidden)]
    const SCHEMA_CHECKS: () = ();

    fn decode(value: Value<'de>) -> Result<Self, Error>;

    /// What a missing field decodes as, if anything.
    fn absent() -> Option<Self> {
        None
    }

    /// A field, from its value if it was there.
    fn decode_field(value: Option<Value<'de>>) -> Result<Self, Error> {
        match value {
            Some(value) => Self::decode(value),
            None => Self::absent().ok_or(Error::new(ErrorKind::Missing)),
        }
    }

    /// A list, as `Encode::encode_list` wrote it.
    fn decode_list(value: Value<'de>) -> Result<Vec<Self>, Error> {
        let mut items = ItemsReader::of(value, Self::WIRE)?;
        charge_items::<Self>(items.len())?;
        let mut decoded = Vec::with_capacity(items.len());
        while let Some(item) = items.next() {
            decoded.push(Self::decode(item?)?);
        }
        items.finish()?;
        Ok(decoded)
    }
}

/// `value` encoded alone, as `decode` reads it.
pub fn encode<T: Encode + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
    let mut w = Writer::new().outermost(T::WIRE);
    value.encode(&mut w)?;
    Ok(w.into_bytes())
}

/// How many bytes `encode` gives, found without writing them.
pub fn encoded_len<T: Encode + ?Sized>(value: &T) -> Result<usize, Error> {
    let mut w = Writer::counting().outermost(T::WIRE);
    value.encode(&mut w)?;
    Ok(w.len())
}

/// A `T` from exactly `bytes`, as `encode` wrote it, allocating at most `BUDGET_BASE` plus
/// `BUDGET_PER_BYTE` for each byte.
pub fn decode<'de, T: Decode<'de>>(bytes: &'de [u8]) -> Result<T, Error> {
    let budget = BUDGET_BASE.saturating_add(BUDGET_PER_BYTE.saturating_mul(bytes.len() as u64));
    decode_with_budget(bytes, budget)
}

/// As `decode`, allocating at most `budget` bytes.
pub fn decode_with_budget<'de, T: Decode<'de>>(bytes: &'de [u8], budget: u64) -> Result<T, Error> {
    let _budget = Budget::start(budget);
    T::decode(Value::outermost(T::WIRE, bytes)?)
}

/// As `decode`, refusing any bytes but the ones encoding the value gives: for bytes that are
/// hashed or compared.
pub fn decode_canonical<'de, T: Decode<'de>>(bytes: &'de [u8]) -> Result<T, Error> {
    let value = decode::<T>(bytes)?;
    if encode(&value)? == bytes {
        Ok(value)
    } else {
        Err(Error::new(ErrorKind::NotCanonical))
    }
}

thread_local! {
    static BUDGET: Cell<Option<u64>> = const { Cell::new(None) };
}

/// The allocation budget of the decode in progress on this thread, put back as it was when
/// dropped (a `Kept` decoded inside another decode has its own).
struct Budget(Option<u64>);

impl Budget {
    fn start(budget: u64) -> Self {
        Budget(BUDGET.with(|cell| cell.replace(Some(budget))))
    }
}

impl Drop for Budget {
    fn drop(&mut self) {
        BUDGET.with(|cell| cell.set(self.0));
    }
}

/// Takes `bytes` from the budget of the decode in progress, failing once it is spent. Outside
/// a decode (a type's `decode` called directly) there is no budget.
pub fn charge(bytes: u64) -> Result<(), Error> {
    BUDGET.with(|cell| match cell.get() {
        None => Ok(()),
        Some(left) if left >= bytes => {
            cell.set(Some(left - bytes));
            Ok(())
        }
        Some(_) => Err(Error::new(ErrorKind::OverBudget)),
    })
}

/// Charges for a vector of `count` `T`s.
pub fn charge_items<T>(count: usize) -> Result<(), Error> {
    charge((count as u64).saturating_mul(size_of::<T>().max(1) as u64))
}

/// A field or variant a type does not know: its id, unless it is marked must-understand.
pub fn understood(key: Key) -> Result<u32, Error> {
    if key.critical {
        Err(Error::new(ErrorKind::NotUnderstood(key.id)))
    } else {
        Ok(key.id)
    }
}

/// The one field of an enum's message: the variant's key and value.
pub fn variant(value: Value<'_>) -> Result<(Key, Value<'_>), Error> {
    let mut reader = Reader::of(value)?;
    if reader.is_empty() {
        return Err(Error::new(ErrorKind::NotOneVariant));
    }
    let field = reader.field()?;
    if !reader.is_empty() {
        return Err(Error::new(ErrorKind::NotOneVariant));
    }
    Ok(field)
}

/// Stores `value` as the field `id` read in a message, refusing it a second time.
pub fn once<'de>(slot: &mut Option<Value<'de>>, id: u32, value: Value<'de>) -> Result<(), Error> {
    match slot.replace(value) {
        Some(_) => Err(Error::new(ErrorKind::Repeated(id))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod frozen_example;
#[cfg(test)]
mod tests;
