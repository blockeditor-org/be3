//! Fuzzing the format, for the fuzz target (fuzz/decode.rs) and the tests: arbitrary bytes
//! never panic when decoded, what decodes encodes canonically, and a value built from arbitrary
//! bytes reads back as it was written.

use std::collections::BTreeMap;

use crate::{Decode, Schema, Unknown, UnknownVariant, decode, encode};

#[derive(Debug, Schema)]
pub struct FuzzRecord {
    flag: bool,
    small: u8,
    wide: u64,
    signed: i32,
    ratio: f64,
    letter: char,
    name: String,
    data: Vec<u8>,
    numbers: Vec<u16>,
    floats: Vec<f32>,
    maybe: Option<u32>,
    nested: Option<Option<String>>,
    map: BTreeMap<String, Vec<i64>>,
    shape: FuzzShape,
    children: Vec<FuzzRecord>,
}

#[derive(Debug, Schema)]
pub enum FuzzShape {
    #[schema(default)]
    Dot,
    Circle(f64),
    Label {
        text: String,
        size: u16,
    },
    Boxed(Box<FuzzShape>),
}

impl FuzzRecord {
    /// A record drawn from `data`.
    pub fn built(data: &[u8]) -> Self {
        Input(data).record(0)
    }
}

/// `FuzzRecord`'s first field, keeping the rest.
#[derive(Debug, Schema)]
pub struct FuzzKeeping {
    flag: bool,
    #[schema(unknown)]
    rest: Unknown,
}

/// `FuzzShape`'s first variant, keeping the others.
#[derive(Debug, Schema)]
pub enum FuzzOpen {
    Dot,
    #[schema(unknown)]
    Other(UnknownVariant),
}

#[derive(Debug, Schema)]
pub struct FuzzBorrowed<'a> {
    name: &'a str,
    data: &'a [u8],
    numbers: Vec<u16>,
}

/// Decodes `data` as each type. None panics, and what decodes is canonical once encoded:
/// decoding that and encoding it again gives the same bytes.
pub fn decode_any(data: &[u8]) {
    stable::<FuzzRecord>(data);
    stable::<FuzzShape>(data);
    stable::<FuzzKeeping>(data);
    stable::<FuzzOpen>(data);
    stable::<Vec<u64>>(data);
    stable::<BTreeMap<u32, String>>(data);
    stable::<Option<Vec<bool>>>(data);
    if let Ok(value) = decode::<FuzzBorrowed>(data) {
        let once = encode(&value);
        let again: FuzzBorrowed = decode(&once).expect("an encoding reads back");
        assert_eq!(encode(&again), once);
    }
}

fn stable<T: for<'a> Decode<'a>>(data: &[u8]) {
    if let Ok(value) = decode::<T>(data) {
        let once = encode(&value);
        let again: T = decode(&once).expect("an encoding reads back");
        assert_eq!(encode(&again), once);
    }
}

/// Builds a record from `data`, and checks it reads back as written, and that a type knowing
/// only its first field keeps the rest and writes the same bytes.
pub fn round_trip(data: &[u8]) {
    let record = FuzzRecord::built(data);
    let bytes = encode(&record);
    let back: FuzzRecord = decode(&bytes).expect("what was written reads back");
    assert_eq!(encode(&back), bytes);
    let kept: FuzzKeeping = decode(&bytes).expect("unknown fields are kept");
    assert_eq!(encode(&kept), bytes);
    let shape = encode(&record.shape);
    let open: FuzzOpen = decode(&shape).expect("unknown variants are kept");
    assert_eq!(encode(&open), shape);
}

/// Values drawn from bytes, zeros once they run out.
struct Input<'a>(&'a [u8]);

impl Input<'_> {
    fn byte(&mut self) -> u8 {
        let (first, rest) = self.0.split_first().unwrap_or((&0, &[]));
        self.0 = rest;
        *first
    }

    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(std::array::from_fn(|_| self.byte()))
    }

    fn count(&mut self, below: u8) -> usize {
        usize::from(self.byte() % below)
    }

    fn text(&mut self) -> String {
        (0..self.count(6))
            .map(|_| char::from_u32(u32::from(self.byte()) * 37).unwrap_or('x'))
            .collect()
    }

    fn shape(&mut self, depth: u32) -> FuzzShape {
        match self.byte() % 4 {
            1 => FuzzShape::Circle(f64::from_bits(self.u64())),
            2 => FuzzShape::Label {
                text: self.text(),
                size: self.u64() as u16,
            },
            3 if depth < 4 => FuzzShape::Boxed(Box::new(self.shape(depth + 1))),
            _ => FuzzShape::Dot,
        }
    }

    fn record(&mut self, depth: u32) -> FuzzRecord {
        FuzzRecord {
            flag: self.byte() % 2 == 1,
            small: self.byte(),
            wide: self.u64(),
            signed: self.u64() as i32,
            ratio: f64::from_bits(self.u64()),
            letter: char::from_u32(self.u64() as u32 % 0x11_0000).unwrap_or('?'),
            name: self.text(),
            data: (0..self.count(9)).map(|_| self.byte()).collect(),
            numbers: (0..self.count(9)).map(|_| self.u64() as u16).collect(),
            floats: (0..self.count(5))
                .map(|_| f32::from_bits(self.u64() as u32))
                .collect(),
            maybe: (self.byte() % 2 == 1).then(|| self.u64() as u32),
            nested: match self.byte() % 3 {
                0 => None,
                1 => Some(None),
                _ => Some(Some(self.text())),
            },
            map: (0..self.count(4))
                .map(|_| {
                    (
                        self.text(),
                        (0..self.count(4)).map(|_| self.u64() as i64).collect(),
                    )
                })
                .collect(),
            shape: self.shape(0),
            children: if depth < 3 {
                (0..self.count(3)).map(|_| self.record(depth + 1)).collect()
            } else {
                Vec::new()
            },
        }
    }
}
