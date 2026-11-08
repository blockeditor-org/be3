/*
A full-text index kept as bytes: the library the search index block type
(plans/search.md) writes and merges, and the host queries. It does no IO: every
part it reads comes through `Parts::read`, by part name and byte range, and
every part it writes comes back in `Written`, so the block type keeps them as
the block's parts and the host reads them with `read-part`.

The index is a small header (`Index`, encoded with `Index::encode`) over
immutable segments, one per batch, and a document table:

- A segment's parts are its term dictionary (sorted blocks of a few KiB, with
  an fst of each block's first term in the header), its postings (blocks of
  128 documents with skip data and each block's highest term frequency and
  shortest field, for block-max pruning), its positions (apart, read only for
  phrases) and its documents (per local id: the document, its time and field
  lengths). A segment's deletion bitmap is a part of its own, rewritten when a
  batch masks one of its documents.
- The document table, sorted by document and split into chunks named in the
  header, says per document the commit it was indexed at, the part its text
  came from and where its postings are. A removed document keeps a tombstone,
  so a merge of two indexes knows it was removed rather than never seen.

Terms are a field byte (`NAME` or `BODY`) and the case-folded word. A query
matches documents holding every clause (a word, a prefix or a phrase) in their
name or body, ranked by BM25 over each field with names boosted, and by the
newer commit on equal scores.
*/

mod compact;
mod merge;
mod query;
mod segment;
mod table;
mod tokens;
mod varint;

use std::{collections::HashMap, future::Future, ops::Range};

use serde::{Deserialize, Serialize};

pub use query::{Clause, Hit, Query};
pub use tokens::{fold, terms};

/// A random id the caller picks for each batch, compaction or merge; the
/// parts it writes are named under it, so two devices never write the same
/// name.
pub type Id = [u8; 16];

/// A content hash, such as a commit id or the part a document's text came from.
pub type Hash = [u8; 32];

/// A document: a block and a key the block's type gives (empty for a block
/// that is one document).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DocId {
    pub block: Vec<u8>,
    pub key: String,
}

/// The commit a document was indexed at. The later time is the newer, then
/// the larger id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Commit {
    pub time: i64,
    pub id: Hash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PartKind {
    Dictionary,
    Postings,
    Positions,
    Documents,
    /// The deletion bitmap of the segment with this id.
    Deletions(Id),
    /// A chunk of the document table.
    Table(u32),
}

/// The name of one of the index's parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PartName {
    /// The segment, or for deletions and table chunks the batch that wrote it.
    pub origin: Id,
    pub kind: PartKind,
}

impl PartName {
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("a part name encodes")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        postcard::from_bytes(bytes).map_err(|_| Error::Corrupt("part name"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A part could not be read; the caller's message.
    Read(String),
    /// A part's bytes are not what the index wrote.
    Corrupt(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Read(message) => write!(f, "reading an index part: {message}"),
            Error::Corrupt(what) => write!(f, "the index's {what} is corrupt"),
        }
    }
}

impl std::error::Error for Error {}

/// Where the index's parts are read from.
pub trait Parts {
    /// The bytes of `part` in `range`; an end past the part's end reads to
    /// its end.
    fn read(&self, part: &PartName, range: Range<u64>) -> impl Future<Output = Result<Vec<u8>, Error>>;
}

/// Parts held in memory, as a test or a type module that loaded them has.
#[derive(Clone, Debug, Default)]
pub struct MemoryParts {
    pub parts: HashMap<PartName, Vec<u8>>,
}

impl MemoryParts {
    pub fn insert(&mut self, written: &Written) {
        for (name, bytes) in &written.added {
            self.parts.insert(*name, bytes.clone());
        }
    }
}

impl Parts for MemoryParts {
    async fn read(&self, part: &PartName, range: Range<u64>) -> Result<Vec<u8>, Error> {
        let bytes = self
            .parts
            .get(part)
            .ok_or_else(|| Error::Read(format!("no part {part:?}")))?;
        let end = (range.end.min(bytes.len() as u64)) as usize;
        let start = (range.start as usize).min(end);
        Ok(bytes[start..end].to_vec())
    }
}

/// What a change to the index wrote: parts to add, and parts the new header
/// no longer names. The header itself is `Index::encode`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Written {
    pub added: Vec<(PartName, Vec<u8>)>,
    pub dropped: Vec<PartName>,
}

/// The searchable fields of a document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Text {
    pub name: String,
    pub body: String,
}

/// A document indexed at a commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Put {
    pub doc: DocId,
    pub commit: Commit,
    pub source: Option<Hash>,
    /// None when its text is unchanged since it was indexed (its source was
    /// in `search-text`'s `indexed`): only its commit and source move.
    pub text: Option<Text>,
}

/// One batch of changes, which adds at most one segment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Batch {
    pub put: Vec<Put>,
    /// Documents gone at a commit (a block's type no longer answers the key).
    pub removed: Vec<(DocId, Commit)>,
    /// Blocks that left the tree, every document of each.
    pub removed_blocks: Vec<(Vec<u8>, Commit)>,
    /// The graph revision the index has now seen every change up to.
    pub watermark: Option<u64>,
}

/// A document table entry: what the index holds for a document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub doc: DocId,
    pub commit: Commit,
    pub source: Option<Hash>,
    /// None for a removed document.
    place: Option<Place>,
}

impl Entry {
    pub fn is_removed(&self) -> bool {
        self.place.is_none()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Place {
    segment: Id,
    local: u32,
}

const NAME: u8 = 0;
const BODY: u8 = 1;

/// The header of an index: its segments, its document table's chunks and its
/// watermark.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Index {
    watermark: u64,
    segments: Vec<segment::Meta>,
    /// Segments compaction replaced, so a merge does not bring them back.
    retired: Vec<Id>,
    table: Vec<table::ChunkRef>,
}

impl Index {
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        postcard::from_bytes(bytes).map_err(|_| Error::Corrupt("header"))
    }

    pub fn encode(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("an index header encodes")
    }

    pub fn watermark(&self) -> u64 {
        self.watermark
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Every part the header names, the header aside.
    pub fn parts(&self) -> Vec<PartName> {
        let mut names = Vec::new();
        for segment in &self.segments {
            names.extend(segment.parts());
        }
        names.extend(self.table.iter().map(|chunk| chunk.part));
        names
    }

    /// The table entry of a document.
    pub async fn entry(&self, parts: &impl Parts, doc: &DocId) -> Result<Option<Entry>, Error> {
        let mut table = table::Edit::new(&self.table);
        Ok(table.get(parts, doc).await?.cloned())
    }

    /// The table entries of a block's documents, removed ones too.
    pub async fn entries_of(&self, parts: &impl Parts, block: &[u8]) -> Result<Vec<Entry>, Error> {
        let mut table = table::Edit::new(&self.table);
        table.entries_of(parts, block).await
    }

    /// Applies a batch: a new segment for the documents with new text, the
    /// table chunks they change and the deletion bitmaps of what they mask.
    pub async fn apply(&mut self, parts: &impl Parts, origin: Id, batch: Batch) -> Result<Written, Error> {
        let mut written = Written::default();
        let mut table = table::Edit::new(&self.table);
        let mut masks: Vec<Place> = Vec::new();
        let mut put: Vec<Put> = Vec::new();
        for item in batch.put {
            match put.iter_mut().find(|other| other.doc == item.doc) {
                Some(other) if other.commit < item.commit => *other = item,
                Some(_) => {}
                None => put.push(item),
            }
        }
        let mut fresh = Vec::new();
        for item in put {
            let old = table.get(parts, &item.doc).await?.cloned();
            if old.as_ref().is_some_and(|old| old.commit >= item.commit) {
                continue;
            }
            match item.text {
                Some(text) => {
                    if let Some(place) = old.and_then(|old| old.place) {
                        masks.push(place);
                    }
                    fresh.push((item.doc, item.commit, item.source, text));
                }
                None => {
                    if let Some(mut entry) = old.filter(|old| !old.is_removed()) {
                        entry.commit = item.commit;
                        entry.source = item.source;
                        table.put(entry);
                    }
                }
            }
        }
        let mut gone: Vec<(Entry, Commit)> = Vec::new();
        for (doc, commit) in batch.removed {
            if let Some(entry) = table.get(parts, &doc).await? {
                gone.push((entry.clone(), commit));
            }
        }
        for (block, commit) in batch.removed_blocks {
            for entry in table.entries_of(parts, &block).await? {
                gone.push((entry, commit));
            }
        }
        for (mut entry, commit) in gone {
            if entry.commit >= commit || fresh.iter().any(|(doc, ..)| *doc == entry.doc) {
                continue;
            }
            if let Some(place) = entry.place.take() {
                masks.push(place);
            }
            entry.commit = commit;
            entry.source = None;
            table.put(entry);
        }
        if !fresh.is_empty() {
            let (meta, segment_parts, places) = segment::build_from_text(origin, fresh.iter().map(|(doc, commit, _, text)| (doc, commit.time, text)));
            for ((doc, commit, source, _), local) in fresh.into_iter().zip(places) {
                table.put(Entry {
                    doc,
                    commit,
                    source,
                    place: Some(Place { segment: origin, local }),
                });
            }
            written.added.extend(segment_parts);
            self.segments.push(meta);
        }
        self.mask(parts, origin, &masks, &mut written).await?;
        self.table = table.finish(origin, &mut written);
        if let Some(watermark) = batch.watermark {
            self.watermark = watermark;
        }
        Ok(written)
    }

    /// Sets the deletion bits of `places`, rewriting each bitmap they touch.
    async fn mask(&mut self, parts: &impl Parts, origin: Id, places: &[Place], written: &mut Written) -> Result<(), Error> {
        for segment in &mut self.segments {
            let locals: Vec<u32> = places
                .iter()
                .filter(|place| place.segment == segment.id)
                .map(|place| place.local)
                .collect();
            if locals.is_empty() {
                continue;
            }
            let mut bits = segment.deletions(parts).await?;
            for local in locals {
                bits.set(local);
            }
            segment.set_deletions(origin, bits, written);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
