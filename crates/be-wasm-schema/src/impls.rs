//! The format of the standard library's types, and of `Uuid`.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasher, Hash};
use std::num::{
    NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI128, NonZeroU8, NonZeroU16, NonZeroU32,
    NonZeroU64, NonZeroU128,
};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use crate::wire::{ItemsReader, Key, Number, Packed, Reader, Reference, Value, Wire};
use crate::{
    Decode, Encode, Error, ErrorKind, ListForm, Writer, charge, charge_items, once, understood,
};

/// A list of numbers' bytes: their memory on a little-endian machine.
fn number_bytes<T: bytemuck::NoUninit>(items: &[T], w: &mut Writer, swap: impl Fn(&T) -> T) {
    if cfg!(target_endian = "little") {
        w.bytes(bytemuck::cast_slice(items));
    } else {
        for item in items {
            w.bytes(bytemuck::bytes_of(&swap(item)));
        }
    }
}

/// Numbers from their little-endian bytes: one copy on a little-endian machine.
fn numbers_from<T: bytemuck::Pod>(bytes: &[u8], swap: impl Fn(T) -> T) -> Result<Vec<T>, Error> {
    let size = size_of::<T>();
    if bytes.len() % size != 0 {
        return Err(Error::new(ErrorKind::Truncated));
    }
    charge(bytes.len() as u64)?;
    let mut items = vec![T::zeroed(); bytes.len() / size];
    bytemuck::cast_slice_mut(&mut items).copy_from_slice(bytes);
    if cfg!(target_endian = "big") {
        for item in &mut items {
            *item = swap(*item);
        }
    }
    Ok(items)
}

/// Packed numbers of a narrower type than `T`, each widened by `widen` from its bytes.
fn widened<T>(packed: Packed<'_>, widen: impl Fn(&[u8]) -> Option<T>) -> Result<Vec<T>, Error> {
    let size = packed.number.size();
    charge_items::<T>(packed.bytes.len() / size)?;
    packed
        .bytes
        .chunks_exact(size)
        .map(|item| widen(item).ok_or(Error::new(ErrorKind::OutOfRange)))
        .collect()
}

fn le_unsigned(bytes: &[u8]) -> u128 {
    let mut wide = [0u8; 16];
    wide[..bytes.len()].copy_from_slice(bytes);
    u128::from_le_bytes(wide)
}

fn le_signed(bytes: &[u8]) -> i128 {
    let negative = bytes.last().is_some_and(|byte| byte & 0x80 != 0);
    let mut wide = [if negative { 0xff } else { 0 }; 16];
    wide[..bytes.len()].copy_from_slice(bytes);
    i128::from_le_bytes(wide)
}

fn number_mismatch(expected: Number, found: Number) -> Error {
    Error::new(ErrorKind::Number { expected, found })
}

/// A fixed-width value, refused when it is wider than `size` bytes.
fn no_wider(value: &Value<'_>, size: usize, wire: Wire) -> Result<(), Error> {
    match value.wire().fixed_size() {
        Some(width) if width <= size && value.wire() != Wire::Empty => Ok(()),
        _ => Err(value.mismatch(wire)),
    }
}

macro_rules! integers {
    ($($ty:ty: $wire:ident, $number:ident, $kind:ident, [$($narrower:ident),*];)*) => {$(
        impl Encode for $ty {
            const WIRE: Wire = Wire::$wire;
            const FROZEN: bool = true;
            const LIST: ListForm = ListForm::Packed(Number::$number);
            fn encode(&self, w: &mut Writer) -> Result<(), Error> {
                w.bytes(&self.to_le_bytes());
                Ok(())
            }
            fn is_absent(&self) -> bool {
                *self == 0
            }
            fn encode_list(items: &[Self], w: &mut Writer) -> Result<(), Error> {
                w.bytes(&[Number::$number as u8]);
                number_bytes(items, w, |item| item.swap_bytes());
                Ok(())
            }
        }
        impl<'de> Decode<'de> for $ty {
            const OPTIONAL: bool = true;
            fn decode(value: Value<'de>) -> Result<Self, Error> {
                no_wider(&value, size_of::<$ty>(), Wire::$wire)?;
                integers!(@$kind value, $ty)
            }
            fn absent() -> Option<Self> {
                Some(0)
            }
            fn decode_list(value: Value<'de>) -> Result<Vec<Self>, Error> {
                let packed = value.packed()?;
                match packed.number {
                    Number::$number => numbers_from(packed.bytes, <$ty>::swap_bytes),
                    $(Number::$narrower => widened(packed, |item| {
                        integers!(@widen $kind item, $ty)
                    }),)*
                    found => Err(number_mismatch(Number::$number, found)),
                }
            }
        }
    )*};
    (@unsigned $value:ident, $ty:ty) => {
        <$ty>::try_from($value.unsigned()?).map_err(|_| Error::new(ErrorKind::OutOfRange))
    };
    (@signed $value:ident, $ty:ty) => {
        <$ty>::try_from($value.signed()?).map_err(|_| Error::new(ErrorKind::OutOfRange))
    };
    (@widen unsigned $item:ident, $ty:ty) => {
        <$ty>::try_from(le_unsigned($item)).ok()
    };
    (@widen signed $item:ident, $ty:ty) => {
        <$ty>::try_from(le_signed($item)).ok()
    };
}

integers! {
    i8: Fixed8, I8, signed, [];
    u16: Fixed16, U16, unsigned, [];
    i16: Fixed16, I16, signed, [I8];
    u32: Fixed32, U32, unsigned, [U16];
    i32: Fixed32, I32, signed, [I8, I16];
    u64: Fixed64, U64, unsigned, [U16, U32];
    i64: Fixed64, I64, signed, [I8, I16, I32];
    u128: Fixed128, U128, unsigned, [U16, U32, U64];
    i128: Fixed128, I128, signed, [I8, I16, I32, I64];
}

/// Bytes: a list of them is `Wire::Bytes`, as they are.
impl Encode for u8 {
    const WIRE: Wire = Wire::Fixed8;
    const FROZEN: bool = true;
    const LIST: ListForm = ListForm::Bytes;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        w.bytes(&[*self]);
        Ok(())
    }

    fn is_absent(&self) -> bool {
        *self == 0
    }

    fn encode_list(items: &[Self], w: &mut Writer) -> Result<(), Error> {
        w.bytes(items);
        Ok(())
    }
}

impl<'de> Decode<'de> for u8 {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        match value {
            Value::Fixed8(value) => Ok(value),
            other => Err(other.mismatch(Wire::Fixed8)),
        }
    }

    fn absent() -> Option<Self> {
        Some(0)
    }

    fn decode_list(value: Value<'de>) -> Result<Vec<Self>, Error> {
        let bytes = value.bytes()?;
        charge(bytes.len() as u64)?;
        Ok(bytes.to_vec())
    }
}

macro_rules! floats {
    ($($ty:ty: $wire:ident, $number:ident;)*) => {$(
        /// Every NaN is written as `NAN`, so equal values are equal bytes.
        impl Encode for $ty {
            const WIRE: Wire = Wire::$wire;
            const FROZEN: bool = true;
            const LIST: ListForm = ListForm::Packed(Number::$number);
            fn encode(&self, w: &mut Writer) -> Result<(), Error> {
                let value = if self.is_nan() { <$ty>::NAN } else { *self };
                w.bytes(&value.to_bits().to_le_bytes());
                Ok(())
            }
            fn is_absent(&self) -> bool {
                self.to_bits() == 0
            }
            fn encode_list(items: &[Self], w: &mut Writer) -> Result<(), Error> {
                w.bytes(&[Number::$number as u8]);
                if items.iter().any(|item| item.is_nan()) {
                    for item in items {
                        item.encode(w)?;
                    }
                } else {
                    number_bytes(items, w, |item| <$ty>::from_bits(item.to_bits().swap_bytes()));
                }
                Ok(())
            }
        }
    )*};
}

floats! {
    f32: Fixed32, F32;
    f64: Fixed64, F64;
}

impl<'de> Decode<'de> for f32 {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        match value {
            Value::Fixed32(bits) => Ok(f32::from_bits(bits)),
            other => Err(other.mismatch(Wire::Fixed32)),
        }
    }

    fn absent() -> Option<Self> {
        Some(0.0)
    }

    fn decode_list(value: Value<'de>) -> Result<Vec<Self>, Error> {
        let packed = value.packed()?;
        match packed.number {
            Number::F32 => numbers_from(packed.bytes, |item: f32| {
                f32::from_bits(item.to_bits().swap_bytes())
            }),
            found => Err(number_mismatch(Number::F32, found)),
        }
    }
}

/// Widens an `f32`, as a field widened from `f32` reads its old values.
impl<'de> Decode<'de> for f64 {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        match value {
            Value::Fixed64(bits) => Ok(f64::from_bits(bits)),
            Value::Fixed32(bits) => Ok(f32::from_bits(bits).into()),
            other => Err(other.mismatch(Wire::Fixed64)),
        }
    }

    fn absent() -> Option<Self> {
        Some(0.0)
    }

    fn decode_list(value: Value<'de>) -> Result<Vec<Self>, Error> {
        let packed = value.packed()?;
        match packed.number {
            Number::F64 => numbers_from(packed.bytes, |item: f64| {
                f64::from_bits(item.to_bits().swap_bytes())
            }),
            Number::F32 => widened(packed, |item| {
                Some(f32::from_le_bytes(item.try_into().ok()?).into())
            }),
            found => Err(number_mismatch(Number::F64, found)),
        }
    }
}

impl Encode for bool {
    const WIRE: Wire = Wire::Fixed8;
    const FROZEN: bool = true;
    const LIST: ListForm = ListForm::Packed(Number::Bool);

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        w.bytes(&[u8::from(*self)]);
        Ok(())
    }

    fn is_absent(&self) -> bool {
        !*self
    }

    fn encode_list(items: &[Self], w: &mut Writer) -> Result<(), Error> {
        w.bytes(&[Number::Bool as u8]);
        w.bytes(bytemuck::cast_slice(items));
        Ok(())
    }
}

impl<'de> Decode<'de> for bool {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        match value {
            Value::Fixed8(0) => Ok(false),
            Value::Fixed8(1) => Ok(true),
            Value::Fixed8(_) => Err(Error::new(ErrorKind::OutOfRange)),
            other => Err(other.mismatch(Wire::Fixed8)),
        }
    }

    fn absent() -> Option<Self> {
        Some(false)
    }

    fn decode_list(value: Value<'de>) -> Result<Vec<Self>, Error> {
        let packed = value.packed()?;
        if packed.number != Number::Bool {
            return Err(number_mismatch(Number::Bool, packed.number));
        }
        widened(packed, |item| match item {
            [0] => Some(false),
            [1] => Some(true),
            _ => None,
        })
    }
}

impl Encode for char {
    const WIRE: Wire = Wire::Fixed32;
    const FROZEN: bool = true;
    const LIST: ListForm = ListForm::Packed(Number::Char);

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        w.bytes(&u32::from(*self).to_le_bytes());
        Ok(())
    }

    fn is_absent(&self) -> bool {
        *self == '\0'
    }

    fn encode_list(items: &[Self], w: &mut Writer) -> Result<(), Error> {
        w.bytes(&[Number::Char as u8]);
        if cfg!(target_endian = "little") {
            w.bytes(bytemuck::cast_slice(items));
        } else {
            for item in items {
                w.bytes(&u32::from(*item).to_le_bytes());
            }
        }
        Ok(())
    }
}

impl<'de> Decode<'de> for char {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        match value {
            Value::Fixed32(value) => char::from_u32(value).ok_or(Error::new(ErrorKind::OutOfRange)),
            other => Err(other.mismatch(Wire::Fixed32)),
        }
    }

    fn absent() -> Option<Self> {
        Some('\0')
    }

    fn decode_list(value: Value<'de>) -> Result<Vec<Self>, Error> {
        let packed = value.packed()?;
        if packed.number != Number::Char {
            return Err(number_mismatch(Number::Char, packed.number));
        }
        widened(packed, |item| char::from_u32(le_unsigned(item) as u32))
    }
}

macro_rules! nonzero {
    ($($ty:ty: $inner:ty;)*) => {$(
        impl Encode for $ty {
            const WIRE: Wire = <$inner as Encode>::WIRE;
            const FROZEN: bool = true;
            fn encode(&self, w: &mut Writer) -> Result<(), Error> {
                self.get().encode(w)
            }
        }
        impl<'de> Decode<'de> for $ty {
            fn decode(value: Value<'de>) -> Result<Self, Error> {
                <$ty>::new(<$inner>::decode(value)?).ok_or(Error::new(ErrorKind::OutOfRange))
            }
        }
    )*};
}

nonzero! {
    NonZeroU8: u8;
    NonZeroU16: u16;
    NonZeroU32: u32;
    NonZeroU64: u64;
    NonZeroU128: u128;
    NonZeroI8: i8;
    NonZeroI16: i16;
    NonZeroI32: i32;
    NonZeroI64: i64;
    NonZeroI128: i128;
}

/// Nothing: written as no bytes.
impl Encode for () {
    const WIRE: Wire = Wire::Empty;
    const FROZEN: bool = true;

    fn encode(&self, _: &mut Writer) -> Result<(), Error> {
        Ok(())
    }

    fn is_absent(&self) -> bool {
        true
    }
}

impl<'de> Decode<'de> for () {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        match value {
            Value::Empty => Ok(()),
            other => Err(other.mismatch(Wire::Empty)),
        }
    }

    fn absent() -> Option<Self> {
        Some(())
    }
}

macro_rules! text {
    ($($ty:ty),*) => {$(
        impl Encode for $ty {
            const WIRE: Wire = Wire::Bytes;
            const FROZEN: bool = true;
            fn encode(&self, w: &mut Writer) -> Result<(), Error> {
                w.bytes(self.as_bytes());
                Ok(())
            }
            fn is_absent(&self) -> bool {
                self.is_empty()
            }
        }
    )*};
}

text!(str, String, &str, Cow<'_, str>);

impl<'de> Decode<'de> for &'de str {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        std::str::from_utf8(value.bytes()?).map_err(|_| Error::new(ErrorKind::Utf8))
    }

    fn absent() -> Option<Self> {
        Some("")
    }
}

impl<'de> Decode<'de> for String {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        let text = <&str>::decode(value)?;
        charge(text.len() as u64)?;
        Ok(text.to_owned())
    }

    fn absent() -> Option<Self> {
        Some(String::new())
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

macro_rules! lists {
    ($($ty:ty),*) => {$(
        impl<T: Encode> Encode for $ty {
            const WIRE: Wire = T::LIST.wire();
            const FROZEN: bool = T::FROZEN;
            fn encode(&self, w: &mut Writer) -> Result<(), Error> {
                T::encode_list(self, w)
            }
            fn is_absent(&self) -> bool {
                self.is_empty()
            }
        }
    )*};
}

// `Box<[T]>` and `Box<str>` encode as `Box<T>` does, through what they hold.
lists!([T], Vec<T>);

impl<'de, T: Decode<'de>> Decode<'de> for Vec<T> {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        T::decode_list(value)
    }

    fn absent() -> Option<Self> {
        Some(Vec::new())
    }
}

/// Bytes, borrowed from the input.
impl Encode for &[u8] {
    const WIRE: Wire = Wire::Bytes;
    const FROZEN: bool = true;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        w.bytes(self);
        Ok(())
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

impl Encode for Cow<'_, [u8]> {
    const WIRE: Wire = Wire::Bytes;
    const FROZEN: bool = true;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        w.bytes(self);
        Ok(())
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<'de> Decode<'de> for Cow<'de, [u8]> {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        value.bytes().map(Cow::Borrowed)
    }

    fn absent() -> Option<Self> {
        Some(Cow::Borrowed(&[]))
    }
}

impl<T: Encode, const N: usize> Encode for [T; N] {
    const WIRE: Wire = T::LIST.wire();
    const FROZEN: bool = T::FROZEN;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        T::encode_list(self, w)
    }
}

impl<'de, T: Decode<'de>, const N: usize> Decode<'de> for [T; N] {
    fn decode(value: Value<'de>) -> Result<Self, Error> {
        T::decode_list(value)?
            .try_into()
            .map_err(|_| Error::new(ErrorKind::OutOfRange))
    }
}

macro_rules! pointers {
    ($($ty:ident),*) => {$(
        impl<T: Encode + ?Sized> Encode for $ty<T> {
            const WIRE: Wire = T::WIRE;
            const FROZEN: bool = T::FROZEN;
            fn encode(&self, w: &mut Writer) -> Result<(), Error> {
                (**self).encode(w)
            }
            fn is_absent(&self) -> bool {
                (**self).is_absent()
            }
            fn encode_field(&self, key: Key, w: &mut Writer) -> Result<(), Error> {
                (**self).encode_field(key, w)
            }
        }
        impl<'de, T: Decode<'de>> Decode<'de> for $ty<T> {
            const OPTIONAL: bool = T::OPTIONAL;
            fn decode(value: Value<'de>) -> Result<Self, Error> {
                charge_items::<T>(1)?;
                T::decode(value).map($ty::new)
            }
            fn absent() -> Option<Self> {
                T::absent().map($ty::new)
            }
            fn decode_field(value: Option<Value<'de>>) -> Result<Self, Error> {
                charge_items::<T>(1)?;
                T::decode_field(value).map($ty::new)
            }
        }
    )*};
}

pointers!(Box, Arc, Rc);

/// As a field, `None` is the field left out and `Some` the field written, even when what it
/// holds is that type's default. Alone or in a list, a message whose field 1 is there or not.
impl<T: Encode> Encode for Option<T> {
    const WIRE: Wire = Wire::Message;
    const FROZEN: bool = T::FROZEN;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        match self {
            Some(value) => w.field(Key::new(1), value),
            None => Ok(()),
        }
    }

    fn is_absent(&self) -> bool {
        self.is_none()
    }

    fn encode_field(&self, key: Key, w: &mut Writer) -> Result<(), Error> {
        match self {
            Some(value) => w.field(key, value),
            None => Ok(()),
        }
    }
}

impl<'de, T: Decode<'de>> Decode<'de> for Option<T> {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        let mut reader = Reader::of(value)?;
        let mut held = None;
        while !reader.is_empty() {
            let (key, value) = reader.field()?;
            if key.id == 1 {
                once(&mut held, 1, value)?;
            } else {
                understood(key)?;
            }
        }
        held.map(T::decode).transpose()
    }

    fn absent() -> Option<Self> {
        Some(None)
    }

    fn decode_field(value: Option<Value<'de>>) -> Result<Self, Error> {
        value.map(T::decode).transpose()
    }
}

macro_rules! tuples {
    ($(($($name:ident $id:literal),*);)*) => {$(
        /// A message of the fields 1, 2, ... in order.
        impl<$($name: Encode),*> Encode for ($($name,)*) {
            const WIRE: Wire = Wire::Message;
            const FROZEN: bool = true $(&& $name::FROZEN)*;
            #[allow(non_snake_case)]
            fn encode(&self, w: &mut Writer) -> Result<(), Error> {
                let ($($name,)*) = self;
                $($name.encode_field(Key::new($id), w)?;)*
                Ok(())
            }
        }
        impl<'de, $($name: Decode<'de>),*> Decode<'de> for ($($name,)*) {
            const OPTIONAL: bool = true $(&& $name::OPTIONAL)*;
            #[allow(non_snake_case)]
            fn decode(value: Value<'de>) -> Result<Self, Error> {
                let mut reader = Reader::of(value)?;
                $(let mut $name = None;)*
                while !reader.is_empty() {
                    let (key, value) = reader.field()?;
                    match key.id {
                        $($id => once(&mut $name, $id, value)?,)*
                        _ => {
                            understood(key)?;
                        }
                    }
                }
                Ok(($($name::decode_field($name)?,)*))
            }
            fn absent() -> Option<Self> {
                Some(($($name::absent()?,)*))
            }
        }
    )*};
}

tuples! {
    (A 1, B 2);
    (A 1, B 2, C 3);
    (A 1, B 2, C 3, D 4);
}

/// Each entry encoded apart and sorted by its key's encoded bytes, then written as a list of
/// messages of the key (1) and value (2): the same map is the same bytes whatever holds it.
fn write_map<'a, K: Encode + 'a, V: Encode + 'a>(
    entries: impl ExactSizeIterator<Item = (&'a K, &'a V)>,
    w: &mut Writer,
) -> Result<(), Error> {
    let mut sorted = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let mut apart = w.apart();
        key.encode(&mut apart)?;
        sorted.push((apart.into_bytes(), key, value));
    }
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    w.list_header(Wire::Message, sorted.len())?;
    for (_, key, value) in sorted {
        w.length(Wire::Message, |w| {
            key.encode_field(Key::new(1), w)?;
            value.encode_field(Key::new(2), w)
        })?;
    }
    Ok(())
}

fn read_map<'de, K: Decode<'de>, V: Decode<'de>>(
    value: Value<'de>,
    mut insert: impl FnMut(K, V) -> bool,
) -> Result<(), Error> {
    let mut entries = ItemsReader::of(value, Wire::Message)?;
    charge_items::<(K, V)>(entries.len())?;
    while let Some(entry) = entries.next() {
        let mut reader = Reader::of(entry?)?;
        let (mut key, mut value) = (None, None);
        while !reader.is_empty() {
            match reader.field()? {
                (Key { id: 1, .. }, found) => once(&mut key, 1, found)?,
                (Key { id: 2, .. }, found) => once(&mut value, 2, found)?,
                (other, _) => {
                    understood(other)?;
                }
            }
        }
        let key = K::decode_field(key).map_err(|error| error.within("map key"))?;
        let value = V::decode_field(value).map_err(|error| error.within("map value"))?;
        if !insert(key, value) {
            return Err(Error::new(ErrorKind::RepeatedKey));
        }
    }
    entries.finish()
}

/// Each element encoded apart and sorted by its encoded bytes, then written as a list.
fn write_set<'a, T: Encode + 'a>(
    elements: impl ExactSizeIterator<Item = &'a T>,
    w: &mut Writer,
) -> Result<(), Error> {
    let mut sorted = Vec::with_capacity(elements.len());
    for element in elements {
        let mut apart = w.apart();
        element.encode(&mut apart)?;
        sorted.push(apart.into_bytes());
    }
    sorted.sort();
    w.list_header(T::WIRE, sorted.len())?;
    for element in sorted {
        if T::WIRE.has_length() {
            w.length(T::WIRE, |w| {
                w.bytes(&element);
                Ok(())
            })?;
        } else {
            w.bytes(&element);
        }
    }
    Ok(())
}

fn read_set<'de, T: Decode<'de>>(
    value: Value<'de>,
    mut insert: impl FnMut(T) -> bool,
) -> Result<(), Error> {
    let mut elements = ItemsReader::of(value, T::WIRE)?;
    charge_items::<T>(elements.len())?;
    while let Some(element) = elements.next() {
        if !insert(T::decode(element?)?) {
            return Err(Error::new(ErrorKind::RepeatedKey));
        }
    }
    elements.finish()
}

impl<K: Encode, V: Encode> Encode for BTreeMap<K, V> {
    const WIRE: Wire = Wire::List;
    const FROZEN: bool = K::FROZEN && V::FROZEN;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        write_map(self.iter(), w)
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<'de, K: Decode<'de> + Ord, V: Decode<'de>> Decode<'de> for BTreeMap<K, V> {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        let mut map = BTreeMap::new();
        read_map(value, |key, value| map.insert(key, value).is_none())?;
        Ok(map)
    }

    fn absent() -> Option<Self> {
        Some(BTreeMap::new())
    }
}

impl<K: Encode, V: Encode, S> Encode for HashMap<K, V, S> {
    const WIRE: Wire = Wire::List;
    const FROZEN: bool = K::FROZEN && V::FROZEN;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        write_map(self.iter(), w)
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
        read_map(value, |key, value| map.insert(key, value).is_none())?;
        Ok(map)
    }

    fn absent() -> Option<Self> {
        Some(HashMap::default())
    }
}

impl<T: Encode> Encode for BTreeSet<T> {
    const WIRE: Wire = Wire::List;
    const FROZEN: bool = T::FROZEN;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        write_set(self.iter(), w)
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<'de, T: Decode<'de> + Ord> Decode<'de> for BTreeSet<T> {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        let mut set = BTreeSet::new();
        read_set(value, |element| set.insert(element))?;
        Ok(set)
    }

    fn absent() -> Option<Self> {
        Some(BTreeSet::new())
    }
}

impl<T: Encode, S> Encode for HashSet<T, S> {
    const WIRE: Wire = Wire::List;
    const FROZEN: bool = T::FROZEN;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        write_set(self.iter(), w)
    }

    fn is_absent(&self) -> bool {
        self.is_empty()
    }
}

impl<'de, T: Decode<'de> + Eq + Hash, S: BuildHasher + Default> Decode<'de> for HashSet<T, S> {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        let mut set = HashSet::default();
        read_set(value, |element| set.insert(element))?;
        Ok(set)
    }

    fn absent() -> Option<Self> {
        Some(HashSet::default())
    }
}

/// A message of its seconds (1) and nanoseconds (2).
impl Encode for Duration {
    const WIRE: Wire = Wire::Message;
    const FROZEN: bool = true;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        self.as_secs().encode_field(Key::new(1), w)?;
        self.subsec_nanos().encode_field(Key::new(2), w)
    }

    fn is_absent(&self) -> bool {
        self.is_zero()
    }
}

impl<'de> Decode<'de> for Duration {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        let (seconds, nanos) = <(u64, u32)>::decode(value)?;
        if nanos >= 1_000_000_000 {
            return Err(Error::new(ErrorKind::OutOfRange));
        }
        Ok(Duration::new(seconds, nanos))
    }

    fn absent() -> Option<Self> {
        Some(Duration::ZERO)
    }
}

/// Its 16 bytes, as they are.
impl Encode for Uuid {
    const WIRE: Wire = Wire::Fixed128;
    const FROZEN: bool = true;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        w.bytes(self.as_bytes());
        Ok(())
    }

    fn is_absent(&self) -> bool {
        self.is_nil()
    }
}

impl<'de> Decode<'de> for Uuid {
    const OPTIONAL: bool = true;

    fn decode(value: Value<'de>) -> Result<Self, Error> {
        match value {
            Value::Fixed128(value) => Ok(Uuid::from_bytes(value.to_le_bytes())),
            other => Err(other.mismatch(Wire::Fixed128)),
        }
    }

    fn absent() -> Option<Self> {
        Some(Uuid::nil())
    }
}

impl Encode for Reference {
    const WIRE: Wire = Wire::Reference;
    const FROZEN: bool = true;

    fn encode(&self, w: &mut Writer) -> Result<(), Error> {
        w.bytes(&self.0);
        Ok(())
    }
}

impl<'de> Decode<'de> for Reference {
    fn decode(value: Value<'de>) -> Result<Self, Error> {
        match value {
            Value::Reference(reference) => Ok(reference),
            other => Err(other.mismatch(Wire::Reference)),
        }
    }
}
