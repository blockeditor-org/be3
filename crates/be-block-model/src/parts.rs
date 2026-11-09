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

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use be_block_store::{ChunkerConfig, Hash, LoadedPart, PartRef, split};
use uuid::Uuid;

use crate::{
    Malformed,
    stored::{ID, Stored, encode},
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
/// The most parts a `SPLICE` names before it gets an index level.
const FANOUT: usize = 64;
/// The encoded size of one hash in a `SPLICE` or `PART`.
const REFERENCE: usize = 2 + Hash::LEN;

/// One part of a document's stored form, with the parts its bytes name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentPart {
    hash: Hash,
    bytes: Vec<u8>,
    references: Vec<Uuid>,
    children: Vec<DocumentPart>,
}

impl DocumentPart {
    fn new(value: &Stored, mut children: Vec<DocumentPart>) -> Self {
        let mut seen = std::collections::BTreeSet::new();
        children.retain(|child| seen.insert(child.hash));
        let bytes = encode(value);
        let mut references = Vec::new();
        crate::references::walk(value, &mut references);
        Self {
            hash: Hash::of(&bytes),
            bytes,
            references: crate::references::distinct(references),
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

    /// The blocks this part's own bytes reference, not its children's.
    pub fn references(&self) -> &[Uuid] {
        &self.references
    }

    /// The parts this one names, each once, in the order it names them.
    pub fn children(&self) -> &[DocumentPart] {
        &self.children
    }

    /// The stored bytes: this part's bytes, then each child's, so the root
    /// comes first and every part's bytes precede the parts it names.
    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.bytes);
        for child in &self.children {
            child.write(out);
        }
    }

    /// The part as the vault saves it, whose leaves in order are `write`'s
    /// bytes: a part with children or references is a node of its own bytes
    /// and then its children, listing its references. A part in `saved` is
    /// handed back as it was saved, references and all.
    pub(crate) fn store(self, saved: &HashMap<Hash, PartRef>) -> be_block_store::Part<'static> {
        if let Some(reference) = saved.get(&self.hash) {
            return be_block_store::Part::Reused(*reference);
        }
        let own = be_block_store::Part::Bytes(Cow::Owned(self.bytes));
        if self.children.is_empty() && self.references.is_empty() {
            return own;
        }
        let mut parts = Vec::with_capacity(self.children.len() + 1);
        parts.push(own);
        parts.extend(self.children.into_iter().map(|child| child.store(saved)));
        be_block_store::Part::Node {
            parts,
            references: self.references,
        }
    }

    /// This part and every part below it, depth first, parents before
    /// children. A part named from two places appears twice.
    pub fn all(&self) -> Vec<&DocumentPart> {
        let mut out = Vec::new();
        let mut stack = vec![self];
        while let Some(part) = stack.pop() {
            out.push(part);
            stack.extend(part.children.iter().rev());
        }
        out
    }
}

pub(crate) fn split_document(document: Stored) -> DocumentPart {
    let mut children = Vec::new();
    let root = shrink(document, &mut children);
    DocumentPart::new(&root.value, children)
}

fn reference(part: &DocumentPart) -> Stored {
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
fn shrink(value: Stored, out: &mut Vec<DocumentPart>) -> Left {
    match value {
        Stored::Bytes(bytes) if bytes.len() > MAX => {
            let whole = header_len(bytes.len()) + bytes.len();
            let parts = split(&bytes, BYTES)
                .into_iter()
                .map(|range| DocumentPart::new(&Stored::Bytes(bytes[range].to_vec()), Vec::new()))
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
                let part = DocumentPart::new(&map, children);
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

fn array(items: Vec<Stored>, out: &mut Vec<DocumentPart>) -> Left {
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
    let level = level(whole / count.max(1));
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
            parts.push(DocumentPart::new(&group, std::mem::take(&mut children)));
            held = 0;
        }
    }
    let (value, size) = splice(parts, out);
    Left { value, size, whole }
}

/// A `SPLICE` naming `parts`, grouped into index parts while it names more
/// than `FANOUT`, so no part has more than a few dozen children.
fn splice(mut parts: Vec<DocumentPart>, out: &mut Vec<DocumentPart>) -> (Stored, usize) {
    loop {
        let size = header(SPLICE) + header_len(parts.len()) + parts.len() * REFERENCE;
        if parts.len() <= FANOUT {
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
                || group.len() + 1 >= 4 * FANOUT
                || ends_part(part.hash.as_bytes(), FANOUT.ilog2() - 1);
            group.push(part);
            if ends {
                let named = Stored::Tag(
                    SPLICE,
                    Box::new(Stored::Array(group.iter().map(reference).collect())),
                );
                index.push(DocumentPart::new(&named, std::mem::take(&mut group)));
            }
        }
        parts = index;
    }
}

/// Reads a document's stored bytes, written by `DocumentPart::write`: a CBOR
/// sequence of parts, the root first. Returns the document as if it had been
/// saved as one value, `{"format": N, "root": object}`.
///
/// `write` puts each part's children right after it, each once, in the order
/// the part first names them, so a name is found where its part sits, and
/// the part's bytes must hash to it.
pub fn read(bytes: &[u8]) -> Result<Stored, Malformed> {
    let mut rest = bytes;
    let (root, _) = next_part(&mut rest)?;
    let whole = resolve(root, &mut rest)?;
    match rest.is_empty() {
        true => Ok(whole),
        false => Err(Malformed),
    }
}

/// The next part in `rest`, and its bytes.
fn next_part<'a>(rest: &mut &'a [u8]) -> Result<(Stored, &'a [u8]), Malformed> {
    let before: &'a [u8] = rest;
    if before.is_empty() {
        return Err(Malformed);
    }
    let value = ciborium::from_reader(&mut *rest).map_err(|_| Malformed)?;
    Ok((value, &before[..before.len() - rest.len()]))
}

/// Puts back into a part's `value` every part it names, read from `rest`.
fn resolve(value: Stored, rest: &mut &[u8]) -> Result<Stored, Malformed> {
    let mut names = HashMap::new();
    count_names(&value, &mut names)?;
    let mut parts = Parts {
        repeated: names
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .map(|(hash, _)| hash)
            .collect(),
        held: HashMap::new(),
    };
    join(value, rest, &mut parts)
}

/// The parts one part names: those it names more than once, kept after the
/// first for the rest.
struct Parts {
    repeated: HashSet<Hash>,
    held: HashMap<Hash, Stored>,
}

fn count_names(value: &Stored, names: &mut HashMap<Hash, usize>) -> Result<(), Malformed> {
    match value {
        Stored::Tag(PART, inner) => *names.entry(hash_in(inner)?).or_default() += 1,
        Stored::Tag(SPLICE, inner) => {
            let Stored::Array(named) = &**inner else {
                return Err(Malformed);
            };
            for name in named {
                *names.entry(hash_in(name)?).or_default() += 1;
            }
        }
        Stored::Tag(_, inner) => count_names(inner, names)?,
        Stored::Array(items) => {
            for item in items {
                count_names(item, names)?;
            }
        }
        Stored::Map(entries) => {
            for (key, value) in entries {
                count_names(key, names)?;
                count_names(value, names)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn join(value: Stored, rest: &mut &[u8], parts: &mut Parts) -> Result<Stored, Malformed> {
    Ok(match value {
        Stored::Tag(PART, inner) => named(&inner, rest, parts)?,
        Stored::Tag(SPLICE, inner) => {
            let Stored::Array(names) = *inner else {
                return Err(Malformed);
            };
            let mut joined: Option<Stored> = None;
            for name in &names {
                let part = named(name, rest, parts)?;
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
        Stored::Tag(tag, inner) => Stored::Tag(tag, Box::new(join(*inner, rest, parts)?)),
        Stored::Array(items) => Stored::Array(
            items
                .into_iter()
                .map(|item| join(item, rest, parts))
                .collect::<Result<_, _>>()?,
        ),
        Stored::Map(entries) => Stored::Map(
            entries
                .into_iter()
                .map(|(key, value)| Ok((join(key, rest, parts)?, join(value, rest, parts)?)))
                .collect::<Result<_, Malformed>>()?,
        ),
        other => other,
    })
}

fn hash_in(name: &Stored) -> Result<Hash, Malformed> {
    let bytes: [u8; Hash::LEN] = name
        .as_bytes()
        .and_then(|bytes| bytes.as_slice().try_into().ok())
        .ok_or(Malformed)?;
    Ok(Hash::from_bytes(bytes))
}

/// The part `name` names, whole: the next one in `rest` the first time, and
/// the one read then if it is named again.
fn named(name: &Stored, rest: &mut &[u8], parts: &mut Parts) -> Result<Stored, Malformed> {
    let hash = hash_in(name)?;
    if let Some(held) = parts.held.get(&hash) {
        return Ok(held.clone());
    }
    let (part, bytes) = next_part(rest)?;
    if Hash::of(bytes) != hash {
        return Err(Malformed);
    }
    let part = resolve(part, rest)?;
    if parts.repeated.contains(&hash) {
        parts.held.insert(hash, part.clone());
    }
    Ok(part)
}

/// What a saved document's parts were saved as, by the hash of their bytes, so
/// a save can hand back those that didn't change.
pub(crate) fn saved(stored: &be_block_store::Stored) -> HashMap<Hash, PartRef> {
    let mut saved = HashMap::new();
    if let Some(root) = &stored.root {
        learn(stored, root, &mut saved);
    }
    saved
}

/// A part with children or references was saved as a node of its own bytes
/// then each child; one without, as its bytes.
fn learn(stored: &be_block_store::Stored, loaded: &LoadedPart, saved: &mut HashMap<Hash, PartRef>) {
    match loaded.reference {
        PartRef::Node(_) => {
            let Some((own, children)) = loaded.children.split_first() else {
                return;
            };
            if !own.children.is_empty() {
                return;
            }
            saved.insert(Hash::of(stored.bytes_of(own)), loaded.reference);
            for child in children {
                learn(stored, child, saved);
            }
        }
        PartRef::Chunk(_) | PartRef::Chunks(_) => {
            saved.insert(Hash::of(stored.bytes_of(loaded)), loaded.reference);
        }
    }
}
