//! One immutable batch of documents: its dictionary, postings, positions and
//! documents parts, and the header's `Meta` for it.
//!
//! - The dictionary is sorted blocks of entries front-coded against the one
//!   before (shared length, suffix, then the term's `TermInfo`), each block
//!   starting a new one past `DICTIONARY_BLOCK` bytes. `Meta::top` is an fst
//!   of every block's first term to its index and `Meta::blocks` the blocks'
//!   offsets, so a lookup reads one block.
//! - A term's postings are its skip data (per block of `POSTINGS_BLOCK`: its
//!   last document, byte length, positions' byte length, highest frequency and
//!   smallest field length) and then the blocks (per document: the delta from
//!   the one before, the frequency, the field length's `norm` byte).
//! - A term's positions, per document in postings order, are deltas.
//! - The documents part is a count, `count + 1` little-endian u32 offsets into
//!   the records, then the records (postcard `Document`s). Local ids are in
//!   newest-first order, so the lower id of two equal scores is the newer.

use std::collections::BTreeMap;

use fst::{IntoStreamer, Map, Streamer};
use serde::{Deserialize, Serialize};

use crate::{DocId, Error, Id, PartKind, PartName, Parts, Text, Written, tokens, varint};

/// Parts with their bytes, as `Written::added` holds them.
pub(crate) type Named = Vec<(PartName, Vec<u8>)>;

pub(crate) const POSTINGS_BLOCK: usize = 128;
const DICTIONARY_BLOCK: usize = 4096;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Meta {
    pub(crate) id: Id,
    pub(crate) docs: u32,
    pub(crate) deleted: u32,
    /// The sum of each field's lengths over the segment's documents.
    pub(crate) lengths: [u64; 2],
    pub(crate) top: Vec<u8>,
    pub(crate) blocks: Vec<u32>,
    pub(crate) deletions: Option<PartName>,
}

impl Meta {
    pub(crate) fn part(&self, kind: PartKind) -> PartName {
        PartName {
            origin: self.id,
            kind,
        }
    }

    pub(crate) fn parts(&self) -> Vec<PartName> {
        let mut names: Vec<PartName> = [
            PartKind::Dictionary,
            PartKind::Postings,
            PartKind::Positions,
            PartKind::Documents,
        ]
        .into_iter()
        .map(|kind| self.part(kind))
        .collect();
        names.extend(self.deletions);
        names
    }

    pub(crate) fn live(&self) -> u32 {
        self.docs - self.deleted
    }

    pub(crate) async fn deletions(&self, parts: &impl Parts) -> Result<Bits, Error> {
        match &self.deletions {
            Some(name) => Ok(Bits(parts.read(name, 0..u64::MAX).await?)),
            None => Ok(Bits::default()),
        }
    }

    pub(crate) fn set_deletions(&mut self, origin: Id, bits: Bits, written: &mut Written) {
        let name = PartName {
            origin,
            kind: PartKind::Deletions(self.id),
        };
        if let Some(old) = self.deletions.replace(name)
            && old != name
        {
            written.dropped.push(old);
        }
        self.deleted = bits.count();
        written.added.retain(|(added, _)| *added != name);
        written.added.push((name, bits.0));
    }

    pub(crate) fn dictionary(&self) -> Result<Dictionary<'_>, Error> {
        Ok(Dictionary {
            meta: self,
            top: Map::new(self.top.as_slice())
                .map_err(|_| Error::Corrupt("top-level dictionary"))?,
        })
    }

    pub(crate) async fn document(&self, parts: &impl Parts, local: u32) -> Result<Document, Error> {
        let name = self.part(PartKind::Documents);
        let at = 4 + 4 * u64::from(local);
        let offsets = parts.read(&name, at..at + 8).await?;
        let (start, end) = (le(&offsets, 0)?, le(&offsets, 4)?);
        let base = 4 + 4 * (u64::from(self.docs) + 1);
        let record = parts
            .read(&name, base + u64::from(start)..base + u64::from(end))
            .await?;
        postcard::from_bytes(&record).map_err(|_| Error::Corrupt("document"))
    }

    pub(crate) async fn documents(&self, parts: &impl Parts) -> Result<Vec<Document>, Error> {
        let bytes = parts
            .read(&self.part(PartKind::Documents), 0..u64::MAX)
            .await?;
        let count = le(&bytes, 0)? as usize;
        let base = 4 + 4 * (count + 1);
        (0..count)
            .map(|local| {
                let (start, end) = (
                    le(&bytes, 4 + 4 * local)? as usize,
                    le(&bytes, 8 + 4 * local)? as usize,
                );
                let record = bytes
                    .get(base + start..base + end)
                    .ok_or(Error::Corrupt("documents"))?;
                postcard::from_bytes(record).map_err(|_| Error::Corrupt("document"))
            })
            .collect()
    }

    /// Every term with its postings and positions, for compaction.
    pub(crate) async fn read_all(
        &self,
        parts: &impl Parts,
    ) -> Result<BTreeMap<Vec<u8>, Vec<Posting>>, Error> {
        let dictionary = parts
            .read(&self.part(PartKind::Dictionary), 0..u64::MAX)
            .await?;
        let postings = parts
            .read(&self.part(PartKind::Postings), 0..u64::MAX)
            .await?;
        let positions = parts
            .read(&self.part(PartKind::Positions), 0..u64::MAX)
            .await?;
        let mut out = BTreeMap::new();
        for pair in self.blocks.windows(2) {
            let block = dictionary
                .get(pair[0] as usize..pair[1] as usize)
                .ok_or(Error::Corrupt("dictionary"))?;
            for (term, info) in decode_dictionary_block(block)? {
                let slice = |bytes: &[u8], start: u64, len: u64| -> Result<Vec<u8>, Error> {
                    bytes
                        .get(start as usize..(start + len) as usize)
                        .map(<[u8]>::to_vec)
                        .ok_or(Error::Corrupt("postings"))
                };
                let skip =
                    decode_skip(&slice(&postings, info.postings, u64::from(info.skip_len))?)?;
                let mut list = Vec::with_capacity(info.df as usize);
                let mut at = info.postings + u64::from(info.skip_len);
                let mut positions_at = info.positions;
                let mut previous = 0;
                for (index, entry) in skip.iter().enumerate() {
                    let count = block_count(info.df, index);
                    let mut block = decode_block(
                        &slice(&postings, at, u64::from(entry.len))?,
                        count,
                        previous,
                    )?;
                    decode_positions(
                        &slice(&positions, positions_at, u64::from(entry.positions_len))?,
                        &mut block,
                    )?;
                    at += u64::from(entry.len);
                    positions_at += u64::from(entry.positions_len);
                    previous = entry.last;
                    list.extend(block);
                }
                out.insert(term, list);
            }
        }
        Ok(out)
    }
}

fn le(bytes: &[u8], at: usize) -> Result<u32, Error> {
    let slice = bytes.get(at..at + 4).ok_or(Error::Corrupt("offsets"))?;
    Ok(u32::from_le_bytes(slice.try_into().unwrap()))
}

/// A deletion bitmap: bit `local` set when that document is masked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Bits(pub(crate) Vec<u8>);

impl Bits {
    pub(crate) fn get(&self, local: u32) -> bool {
        self.0
            .get(local as usize / 8)
            .is_some_and(|byte| byte & (1 << (local % 8)) != 0)
    }

    pub(crate) fn set(&mut self, local: u32) {
        let index = local as usize / 8;
        if self.0.len() <= index {
            self.0.resize(index + 1, 0);
        }
        self.0[index] |= 1 << (local % 8);
    }

    pub(crate) fn union(&mut self, other: &Bits) {
        if self.0.len() < other.0.len() {
            self.0.resize(other.0.len(), 0);
        }
        for (byte, other) in self.0.iter_mut().zip(&other.0) {
            *byte |= other;
        }
    }

    pub(crate) fn count(&self) -> u32 {
        self.0.iter().map(|byte| byte.count_ones()).sum()
    }
}

/// A document's record in its segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Document {
    pub(crate) doc: DocId,
    pub(crate) time: i64,
    pub(crate) lengths: [u32; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Posting {
    pub(crate) doc: u32,
    pub(crate) tf: u32,
    pub(crate) norm: u8,
    pub(crate) positions: Vec<u32>,
}

/// A field length kept in one byte: exact below 32, then 9% steps.
pub(crate) fn decode_norm(norm: u8) -> f64 {
    if norm < 32 {
        f64::from(norm)
    } else {
        32.0 * 1.09f64.powi(i32::from(norm) - 32)
    }
}

pub(crate) fn encode_norm(len: u32) -> u8 {
    let len = f64::from(len);
    let mut low = 0u8;
    let mut high = 255u8;
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if decode_norm(middle) <= len {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TermInfo {
    pub(crate) df: u32,
    pub(crate) postings: u64,
    pub(crate) skip_len: u32,
    pub(crate) postings_len: u32,
    pub(crate) positions: u64,
}

impl TermInfo {
    fn encode(&self, out: &mut Vec<u8>) {
        for value in [
            u64::from(self.df),
            self.postings,
            u64::from(self.skip_len),
            u64::from(self.postings_len),
            self.positions,
        ] {
            varint::put(out, value);
        }
    }

    fn decode(reader: &mut varint::Reader) -> Result<Self, Error> {
        Ok(Self {
            df: reader.u32()?,
            postings: reader.varint()?,
            skip_len: reader.u32()?,
            postings_len: reader.u32()?,
            positions: reader.varint()?,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Skip {
    pub(crate) last: u32,
    pub(crate) len: u32,
    pub(crate) positions_len: u32,
    pub(crate) max_tf: u32,
    pub(crate) min_norm: u8,
}

pub(crate) fn block_count(df: u32, index: usize) -> usize {
    (df as usize - index * POSTINGS_BLOCK).min(POSTINGS_BLOCK)
}

pub(crate) fn decode_skip(bytes: &[u8]) -> Result<Vec<Skip>, Error> {
    let mut reader = varint::Reader::new(bytes);
    let count = reader.varint()? as usize;
    let mut last = 0;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        last += reader.u32()?;
        out.push(Skip {
            last,
            len: reader.u32()?,
            positions_len: reader.u32()?,
            max_tf: reader.u32()?,
            min_norm: reader.byte()?,
        });
    }
    Ok(out)
}

/// A block's postings, without positions. `previous` is the last document of
/// the block before (0 for the first, whose first delta is its document).
pub(crate) fn decode_block(
    bytes: &[u8],
    count: usize,
    previous: u32,
) -> Result<Vec<Posting>, Error> {
    let mut reader = varint::Reader::new(bytes);
    let mut doc = previous;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        doc += reader.u32()?;
        out.push(Posting {
            doc,
            tf: reader.u32()?,
            norm: reader.byte()?,
            positions: Vec::new(),
        });
    }
    Ok(out)
}

pub(crate) fn decode_positions(bytes: &[u8], block: &mut [Posting]) -> Result<(), Error> {
    let mut reader = varint::Reader::new(bytes);
    for posting in block {
        let mut position = 0;
        posting.positions = (0..posting.tf)
            .map(|_| {
                position += reader.u32()?;
                Ok(position)
            })
            .collect::<Result<_, Error>>()?;
    }
    Ok(())
}

fn decode_dictionary_block(bytes: &[u8]) -> Result<Vec<(Vec<u8>, TermInfo)>, Error> {
    let mut reader = varint::Reader::new(bytes);
    let mut out: Vec<(Vec<u8>, TermInfo)> = Vec::new();
    let mut term: Vec<u8> = Vec::new();
    while !reader.done() {
        let shared = reader.varint()? as usize;
        let suffix = reader.varint()? as usize;
        term.truncate(shared);
        term.extend_from_slice(reader.bytes(suffix)?);
        out.push((term.clone(), TermInfo::decode(&mut reader)?));
    }
    Ok(out)
}

/// A segment's dictionary, read a block at a time.
pub(crate) struct Dictionary<'a> {
    meta: &'a Meta,
    top: Map<&'a [u8]>,
}

impl Dictionary<'_> {
    /// The block that would hold `key`, if any could.
    pub(crate) fn block_of(&self, key: &[u8]) -> Option<usize> {
        let next = self
            .top
            .range()
            .gt(key)
            .into_stream()
            .next()
            .map(|(_, index)| index as usize);
        match next {
            Some(0) => None,
            Some(index) => Some(index - 1),
            None => self.meta.blocks.len().checked_sub(2),
        }
    }

    /// The blocks that may hold terms starting with `prefix`, at most `most`.
    pub(crate) fn prefix_blocks(&self, prefix: &[u8], most: usize) -> std::ops::Range<usize> {
        let start = self.block_of(prefix).unwrap_or(0);
        let mut end = start + 1;
        let mut stream = self.top.range().gt(prefix).into_stream();
        while let Some((first, index)) = stream.next()
            && first.starts_with(prefix)
            && end - start < most
        {
            end = index as usize + 1;
        }
        start..end.min(self.meta.blocks.len().saturating_sub(1))
    }

    pub(crate) async fn read_block(
        &self,
        parts: &impl Parts,
        index: usize,
    ) -> Result<Vec<(Vec<u8>, TermInfo)>, Error> {
        let range = u64::from(self.meta.blocks[index])..u64::from(self.meta.blocks[index + 1]);
        let bytes = parts
            .read(&self.meta.part(PartKind::Dictionary), range)
            .await?;
        decode_dictionary_block(&bytes)
    }
}

/// Builds a segment from documents in local id order and their terms.
pub(crate) fn build(
    id: Id,
    documents: &[Document],
    terms: &BTreeMap<Vec<u8>, Vec<Posting>>,
) -> (Meta, Named) {
    let mut dictionary = Vec::new();
    let mut postings = Vec::new();
    let mut positions = Vec::new();
    let mut blocks = vec![0u32];
    let mut firsts: Vec<Vec<u8>> = Vec::new();
    let mut block_start = 0;
    let mut previous_term: &[u8] = &[];
    for (term, list) in terms {
        let info_start = postings.len() as u64;
        let positions_start = positions.len() as u64;
        let mut skip = Vec::new();
        let mut body = Vec::new();
        varint::put(&mut skip, list.len().div_ceil(POSTINGS_BLOCK) as u64);
        let mut previous_last = 0;
        let mut previous_doc = 0;
        for chunk in list.chunks(POSTINGS_BLOCK) {
            let body_start = body.len();
            let positions_before = positions.len();
            for posting in chunk {
                varint::put(&mut body, u64::from(posting.doc - previous_doc));
                varint::put(&mut body, u64::from(posting.tf));
                body.push(posting.norm);
                previous_doc = posting.doc;
                let mut previous_position = 0;
                for &position in &posting.positions {
                    varint::put(&mut positions, u64::from(position - previous_position));
                    previous_position = position;
                }
            }
            let last = chunk.last().unwrap().doc;
            varint::put(&mut skip, u64::from(last - previous_last));
            varint::put(&mut skip, (body.len() - body_start) as u64);
            varint::put(&mut skip, (positions.len() - positions_before) as u64);
            varint::put(
                &mut skip,
                u64::from(chunk.iter().map(|posting| posting.tf).max().unwrap()),
            );
            skip.push(chunk.iter().map(|posting| posting.norm).min().unwrap());
            previous_last = last;
        }
        let info = TermInfo {
            df: list.len() as u32,
            postings: info_start,
            skip_len: skip.len() as u32,
            postings_len: (skip.len() + body.len()) as u32,
            positions: positions_start,
        };
        postings.extend(skip);
        postings.extend(body);
        if dictionary.len() - block_start >= DICTIONARY_BLOCK {
            blocks.push(dictionary.len() as u32);
            block_start = dictionary.len();
        }
        let shared = if dictionary.len() == block_start {
            firsts.push(term.clone());
            0
        } else {
            previous_term
                .iter()
                .zip(term)
                .take_while(|(a, b)| a == b)
                .count()
        };
        varint::put(&mut dictionary, shared as u64);
        varint::put(&mut dictionary, (term.len() - shared) as u64);
        dictionary.extend_from_slice(&term[shared..]);
        info.encode(&mut dictionary);
        previous_term = term;
    }
    if !dictionary.is_empty() {
        blocks.push(dictionary.len() as u32);
    }
    let top = Map::from_iter(
        firsts
            .iter()
            .enumerate()
            .map(|(index, first)| (first.as_slice(), index as u64)),
    )
    .expect("first terms are sorted and distinct")
    .into_fst()
    .into_inner();
    let mut records = Vec::new();
    let mut offsets = Vec::with_capacity(4 * (documents.len() + 2));
    offsets.extend_from_slice(&(documents.len() as u32).to_le_bytes());
    offsets.extend_from_slice(&0u32.to_le_bytes());
    let mut lengths = [0u64; 2];
    for document in documents {
        records.extend(postcard::to_allocvec(document).expect("a document encodes"));
        offsets.extend_from_slice(&(records.len() as u32).to_le_bytes());
        lengths[0] += u64::from(document.lengths[0]);
        lengths[1] += u64::from(document.lengths[1]);
    }
    offsets.extend(records);
    let meta = Meta {
        id,
        docs: documents.len() as u32,
        deleted: 0,
        lengths,
        top,
        blocks,
        deletions: None,
    };
    let parts = vec![
        (meta.part(PartKind::Dictionary), dictionary),
        (meta.part(PartKind::Postings), postings),
        (meta.part(PartKind::Positions), positions),
        (meta.part(PartKind::Documents), offsets),
    ];
    (meta, parts)
}

/// Orders documents newest first and gives each its local id: the returned
/// vector maps an input index to its local id.
pub(crate) fn assign(documents: &[Document]) -> Vec<u32> {
    let mut order: Vec<usize> = (0..documents.len()).collect();
    order.sort_by(|&a, &b| {
        let (a, b) = (&documents[a], &documents[b]);
        b.time.cmp(&a.time).then_with(|| a.doc.cmp(&b.doc))
    });
    let mut local = vec![0; documents.len()];
    for (index, &input) in order.iter().enumerate() {
        local[input] = index as u32;
    }
    local
}

pub(crate) fn sorted(documents: Vec<Document>, local: &[u32]) -> Vec<Document> {
    let mut slots: Vec<Option<Document>> = vec![None; documents.len()];
    for (document, &local) in documents.into_iter().zip(local) {
        slots[local as usize] = Some(document);
    }
    slots.into_iter().map(Option::unwrap).collect()
}

/// A segment of fresh documents: its meta, parts, and each input's local id.
pub(crate) fn build_from_text<'a>(
    id: Id,
    input: impl Iterator<Item = (&'a DocId, i64, &'a Text)>,
) -> (Meta, Named, Vec<u32>) {
    let mut documents = Vec::new();
    let mut texts = Vec::new();
    for (doc, time, text) in input {
        let (keys, lengths) = tokens::document_terms(text);
        documents.push(Document {
            doc: doc.clone(),
            time,
            lengths,
        });
        texts.push(keys);
    }
    let local = assign(&documents);
    let mut terms: BTreeMap<Vec<u8>, Vec<Posting>> = BTreeMap::new();
    for (index, keys) in texts.into_iter().enumerate() {
        let doc = local[index];
        let lengths = documents[index].lengths;
        let mut per_term: BTreeMap<Vec<u8>, Vec<u32>> = BTreeMap::new();
        for (key, position) in keys {
            per_term.entry(key).or_default().push(position);
        }
        for (key, positions) in per_term {
            let norm = encode_norm(lengths[key[0] as usize]);
            terms.entry(key).or_default().push(Posting {
                doc,
                tf: positions.len() as u32,
                norm,
                positions,
            });
        }
    }
    for list in terms.values_mut() {
        list.sort_by_key(|posting| posting.doc);
    }
    let documents = sorted(documents, &local);
    let (meta, parts) = build(id, &documents, &terms);
    (meta, parts, local)
}
