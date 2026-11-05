//! Splitting a document's stored form into parts, so a save rewrites only the
//! parts holding what changed.
//!
//! A part is CBOR. Where a value moved into parts of its own, its parent holds
//! a tag naming them by the SHA-256 of their bytes:
//!
//! - `PART(hash)` stands for the value the part holds: a list object larger
//!   than a part.
//! - `SPLICE([hash, ...])` stands for the concatenation of the arrays or byte
//!   strings its parts hold. A part it names may itself hold a `SPLICE`, an
//!   index level of a long one.
//!
//! An array splits after items whose key hashes below a threshold, as a prolly
//! tree does: a list object's key is its id, any other item's key is its
//! encoding. A byte string (`Text`, `RichText`'s text, a `Grid`'s cells) splits
//! by content-defined chunking. Both decisions depend only on nearby content,
//! so an edit changes the part holding it and the parts on the path up to the
//! root.

use be_block_store::{ChunkerConfig, Hash, split};

use crate::{
    Malformed,
    stored::{ID, Stored, decode, encode},
};

pub const PART: u64 = 0x6265_3304;
pub const SPLICE: u64 = 0x6265_3305;

/// The part size an array aims for: parts average between half of it and it.
const TARGET: usize = 32 * 1024;
/// An array or byte string larger than this splits.
const MAX: usize = 64 * 1024;
/// A part grows past this only if no item fits a boundary sooner, which with
/// boundaries every `TARGET` on average is rare.
const CAP: usize = 4 * MAX;
const BYTES: ChunkerConfig = ChunkerConfig {
    minimum: 16 * 1024,
    average: 16 * 1024,
    maximum: MAX,
};
/// The encoded size of one hash in a `SPLICE` or `PART`.
const REFERENCE: usize = 2 + Hash::LEN;

/// One part of a document's stored form, with the parts its bytes name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Part {
    hash: Hash,
    bytes: Vec<u8>,
    children: Vec<Part>,
}

impl Part {
    fn new(value: &Stored, mut children: Vec<Part>) -> Self {
        let mut seen = std::collections::BTreeSet::new();
        children.retain(|child| seen.insert(child.hash));
        let bytes = encode(value);
        Self {
            hash: Hash::of(&bytes),
            bytes,
            children,
        }
    }

    /// The SHA-256 of the part's bytes, which is how its parent names it.
    pub fn hash(&self) -> Hash {
        self.hash
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The parts this one names, each once, in the order it names them.
    pub fn children(&self) -> &[Part] {
        &self.children
    }

    /// This part and every part below it, depth first, parents before
    /// children. A part named from two places appears twice.
    pub fn all(&self) -> Vec<&Part> {
        let mut out = Vec::new();
        let mut stack = vec![self];
        while let Some(part) = stack.pop() {
            out.push(part);
            stack.extend(part.children.iter().rev());
        }
        out
    }
}

pub(crate) fn split_document(document: Stored) -> Part {
    let mut children = Vec::new();
    let root = shrink(document, &mut children);
    Part::new(&root.value, children)
}

fn reference(part: &Part) -> Stored {
    Stored::Bytes(part.hash.as_bytes().to_vec())
}

fn header(value: u64) -> usize {
    match value {
        0..24 => 1,
        24..256 => 2,
        256..65536 => 3,
        65536..=0xffff_ffff => 5,
        _ => 9,
    }
}

fn header_len(len: usize) -> usize {
    header(u64::try_from(len).unwrap_or(u64::MAX))
}

/// Whether an item whose key hashes to `key` ends a part when parts hold
/// `2^level` items on average.
fn ends_part(key: &[u8], level: u32) -> bool {
    let hash = Hash::of(key);
    let mut first = [0u8; 8];
    first.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_be_bytes(first).leading_zeros() >= level
}

/// The level that makes parts of items averaging `item` bytes come to between
/// half of `TARGET` and `TARGET`.
fn level(item: usize) -> u32 {
    (TARGET / item.max(1)).max(1).ilog2()
}

/// Whether a list object never ends a part, in any list whose objects are
/// small enough to share parts.
#[cfg(test)]
pub(crate) fn never_ends_part(id: crate::ObjectId) -> bool {
    !ends_part(id.as_uuid().as_bytes(), 1)
}

/// What is left of a value once what is too large moved into parts: its
/// encoded size now, and before.
struct Left {
    value: Stored,
    size: usize,
    whole: usize,
}

impl Left {
    fn same(value: Stored, size: usize) -> Self {
        Self {
            value,
            size,
            whole: size,
        }
    }
}

/// The id a list object is stored with, which is its key in its list.
fn object_id(entries: &[(Stored, Stored)]) -> Option<&[u8]> {
    entries.iter().find_map(|(key, value)| match (key, value) {
        (Stored::Text(key), Stored::Tag(_, id)) if key == ID => id.as_bytes().map(Vec::as_slice),
        _ => None,
    })
}

/// Moves what is too large in `value` into parts, pushed onto `out`, and
/// returns what is left.
fn shrink(value: Stored, out: &mut Vec<Part>) -> Left {
    match value {
        Stored::Bytes(bytes) if bytes.len() > MAX => {
            let whole = header_len(bytes.len()) + bytes.len();
            let parts = split(&bytes, BYTES)
                .into_iter()
                .map(|range| Part::new(&Stored::Bytes(bytes[range].to_vec()), Vec::new()))
                .collect();
            let (value, size) = splice(parts, out);
            Left { value, size, whole }
        }
        Stored::Bytes(bytes) => {
            let size = header_len(bytes.len()) + bytes.len();
            Left::same(Stored::Bytes(bytes), size)
        }
        Stored::Text(text) => {
            let size = header_len(text.len()) + text.len();
            Left::same(Stored::Text(text), size)
        }
        Stored::Array(items) => array(items, out),
        Stored::Map(entries) => {
            let id = object_id(&entries).is_some();
            let mut children = Vec::new();
            let mut size = header_len(entries.len());
            let mut whole = size;
            let entries: Vec<(Stored, Stored)> = entries
                .into_iter()
                .map(|(key, value)| {
                    let key = shrink(key, &mut children);
                    let value = shrink(value, &mut children);
                    size += key.size + value.size;
                    whole += key.whole + value.whole;
                    (key.value, value.value)
                })
                .collect();
            let map = Stored::Map(entries);
            if id && whole > TARGET {
                let part = Part::new(&map, children);
                let tag = Stored::Tag(PART, Box::new(reference(&part)));
                out.push(part);
                return Left {
                    value: tag,
                    size: header(PART) + REFERENCE,
                    whole,
                };
            }
            out.extend(children);
            Left {
                value: map,
                size,
                whole,
            }
        }
        Stored::Tag(tag, inner) => {
            let inner = shrink(*inner, out);
            Left {
                value: Stored::Tag(tag, Box::new(inner.value)),
                size: header(tag) + inner.size,
                whole: header(tag) + inner.whole,
            }
        }
        other => {
            let size = encode(&other).len();
            Left::same(other, size)
        }
    }
}

fn array(items: Vec<Stored>, out: &mut Vec<Part>) -> Left {
    let count = items.len();
    let mut size = header_len(count);
    let mut whole = size;
    let mut shrunk = Vec::with_capacity(count);
    for item in items {
        let key = match &item {
            Stored::Map(entries) => object_id(entries).map(<[u8]>::to_vec),
            _ => None,
        };
        let mut children = Vec::new();
        let left = shrink(item, &mut children);
        size += left.size;
        whole += left.whole;
        shrunk.push((left, key, children));
    }
    if size <= MAX {
        let mut values = Vec::with_capacity(count);
        for (left, _, children) in shrunk {
            out.extend(children);
            values.push(left.value);
        }
        return Left {
            value: Stored::Array(values),
            size,
            whole,
        };
    }
    let level = level(size / count.max(1));
    let mut parts = Vec::new();
    let mut values = Vec::new();
    let mut children = Vec::new();
    let mut held = 0;
    let last = shrunk.len() - 1;
    for (index, (left, key, named)) in shrunk.into_iter().enumerate() {
        held += left.size;
        let ends = held >= CAP
            || index == last
            || match &key {
                Some(key) => ends_part(key, level),
                None => ends_part(&encode(&left.value), level),
            };
        values.push(left.value);
        children.extend(named);
        if ends {
            let group = Stored::Array(std::mem::take(&mut values));
            parts.push(Part::new(&group, std::mem::take(&mut children)));
            held = 0;
        }
    }
    let (value, size) = splice(parts, out);
    Left { value, size, whole }
}

/// A `SPLICE` naming `parts`, grouped into index parts while it is too long to
/// hold in one.
fn splice(mut parts: Vec<Part>, out: &mut Vec<Part>) -> (Stored, usize) {
    let level = level(REFERENCE);
    loop {
        let size = header(SPLICE) + header_len(parts.len()) + parts.len() * REFERENCE;
        if size <= MAX {
            let tag = Stored::Tag(
                SPLICE,
                Box::new(Stored::Array(parts.iter().map(reference).collect())),
            );
            out.extend(parts);
            return (tag, size);
        }
        let mut index = Vec::new();
        let mut group = Vec::new();
        let last = parts.len() - 1;
        for (position, part) in parts.into_iter().enumerate() {
            let ends = position == last
                || (group.len() + 1) * REFERENCE >= CAP
                || ends_part(part.hash.as_bytes(), level);
            group.push(part);
            if ends {
                let named = Stored::Tag(
                    SPLICE,
                    Box::new(Stored::Array(group.iter().map(reference).collect())),
                );
                index.push(Part::new(&named, std::mem::take(&mut group)));
            }
        }
        parts = index;
    }
}

/// Puts back into `value` every part it names, fetched with `fetch`, so it
/// reads as if it had been saved whole.
pub(crate) fn join(
    value: Stored,
    fetch: &mut dyn FnMut(Hash) -> Option<Vec<u8>>,
) -> Result<Stored, Malformed> {
    Ok(match value {
        Stored::Tag(PART, inner) => {
            let part = fetched(&inner, fetch)?;
            join(part, fetch)?
        }
        Stored::Tag(SPLICE, inner) => {
            let Stored::Array(names) = *inner else {
                return Err(Malformed);
            };
            let mut joined: Option<Stored> = None;
            for name in &names {
                let part = join(fetched(name, fetch)?, fetch)?;
                joined = Some(match (joined, part) {
                    (None, part @ (Stored::Bytes(_) | Stored::Array(_))) => part,
                    (Some(Stored::Bytes(mut held)), Stored::Bytes(more)) => {
                        held.extend(more);
                        Stored::Bytes(held)
                    }
                    (Some(Stored::Array(mut held)), Stored::Array(more)) => {
                        held.extend(more);
                        Stored::Array(held)
                    }
                    _ => return Err(Malformed),
                });
            }
            joined.ok_or(Malformed)?
        }
        Stored::Tag(tag, inner) => Stored::Tag(tag, Box::new(join(*inner, fetch)?)),
        Stored::Array(items) => Stored::Array(
            items
                .into_iter()
                .map(|item| join(item, fetch))
                .collect::<Result<_, _>>()?,
        ),
        Stored::Map(entries) => Stored::Map(
            entries
                .into_iter()
                .map(|(key, value)| Ok((key, join(value, fetch)?)))
                .collect::<Result<_, Malformed>>()?,
        ),
        other => other,
    })
}

fn fetched(
    name: &Stored,
    fetch: &mut dyn FnMut(Hash) -> Option<Vec<u8>>,
) -> Result<Stored, Malformed> {
    let bytes: [u8; Hash::LEN] = name
        .as_bytes()
        .and_then(|bytes| bytes.as_slice().try_into().ok())
        .ok_or(Malformed)?;
    let hash = Hash::from_bytes(bytes);
    let part = fetch(hash).ok_or(Malformed)?;
    if Hash::of(&part) != hash {
        return Err(Malformed);
    }
    decode(&part).ok_or(Malformed)
}
