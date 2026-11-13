/*
A full-text index kept as bytes: the library the search index block type
(plans/search.md) writes and merges, and the host queries. It does no IO: every
part it reads comes through `Parts::read`, by part name and byte range, and
every part it writes comes back in `Written`, so the block type keeps them as
the block's parts and the host reads them with `read-part`.

The index is a small header (`Index`, encoded with `Index::encode`) over
immutable segments, one per batch, and a document table:

- A segment's parts are its term dictionary (sorted blocks of a few KiB, with
  each block's first term and highest document frequency in the header), its
  postings (blocks of
  128 documents with skip data and each block's highest term frequency and
  shortest field, for block-max pruning), its positions (apart, read only for
  phrases) and its documents (per local id: the document, its commit and field
  lengths). A segment's deletion bitmap is a part of its own, rewritten when a
  batch masks one of its documents.
- The document table, sorted by document and split into chunks named in the
  header, says per document the commit it was indexed at, the part its text
  came from and where its postings are. A removed document keeps a tombstone,
  so a merge of two indexes knows it was removed rather than never seen, and
  a block that left the tree keeps one in the header for the documents of it
  the index never saw.

Every part may come from another device, so nothing read is trusted: bytes
that are not what the index wrote are `Error::Corrupt`, never a panic or an
allocation larger than the bytes read.

The dictionary's top level is a sorted list rather than an fst: fst reads its
nodes by unchecked index, so a crafted part from another device could panic
the reader, and the list is validated whole when the header is decoded.

Segments, not one index rewritten in place: a batch adds parts without
touching the others, and two devices' batches merge as a union. Rejected for
now: tantivy, which wants a filesystem directory, threads and its own merge
policy, and whose segments the commit DAG cannot three-way merge.

Terms are a field byte (`NAME` or `BODY`) and the case-folded word. A query
matches documents holding every clause (a word, a prefix or a phrase) in their
name or body, ranked by BM25 over each field with names boosted, and by the
newer commit on equal scores.
*/

mod compact;
mod join;
mod merge;
mod query;
mod segment;
mod snippet;
mod table;
mod tokens;
mod varint;

use std::{
    collections::{BTreeMap, HashMap},
    future::Future,
    ops::Range,
};

use serde::{Deserialize, Serialize};

pub use query::{Clause, Hit, Query};
pub use snippet::Snippet;
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

impl Commit {
    /// This commit as a reader whose clock allows times up to `latest` takes it: a time past
    /// that is not believed, and counts as the oldest. Any editor of a root may write its index
    /// and its blocks' commits, so a forged far-future time would otherwise win every
    /// comparison, and its document could never be updated again. Only a comparison key: an
    /// index stores the time as it was given, so a reader whose `latest` is too early never
    /// rewrites an honest one.
    pub fn believed(self, latest: i64) -> Self {
        match self.time > latest {
            true => Self {
                time: i64::MIN,
                ..self
            },
            false => self,
        }
    }
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
    /// its end. Boxed, as a future returned from a trait method in another crate must be
    /// (guides/type_modules.md, "The generator").
    fn read<'a>(
        &'a self,
        part: &'a PartName,
        range: Range<u64>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + 'a>>;
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
    fn read<'a>(
        &'a self,
        part: &'a PartName,
        range: Range<u64>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + 'a>> {
        Box::pin(async move {
            let bytes = self
                .parts
                .get(part)
                .ok_or_else(|| Error::Read(format!("no part {part:?}")))?;
            let end = (range.end.min(bytes.len() as u64)) as usize;
            let start = (range.start as usize).min(end);
            Ok(bytes[start..end].to_vec())
        })
    }
}

/// What a change to the index wrote: parts to add, and parts the new header
/// no longer names. The header itself is `Index::encode`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Written {
    pub added: Vec<(PartName, Vec<u8>)>,
    pub dropped: Vec<PartName>,
    /// Documents whose text the index does not hold for their newest
    /// commit (a `Put` without text it had nothing for, or a merge that
    /// lost it): ask their type for it again.
    pub needs_text: Vec<DocId>,
}

/// The searchable fields of a document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Text {
    pub name: String,
    pub body: String,
}

impl Text {
    /// SHA-256 of the name's length, the name and the body.
    pub fn digest(&self) -> Hash {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update((self.name.len() as u64).to_le_bytes());
        hasher.update(self.name.as_bytes());
        hasher.update(self.body.as_bytes());
        hasher.finalize().into()
    }
}

/// A document indexed at a commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Put {
    pub doc: DocId,
    pub commit: Commit,
    pub source: Option<Hash>,
    /// None when its text is unchanged since it was indexed (its source was
    /// in `search-text`'s `indexed`): only its commit moves. With nothing
    /// indexed for it from that same source, it comes back in
    /// `Written::needs_text`.
    pub text: Option<Text>,
    /// Its text is all a device could read of it, not its type's (the block's name, where no
    /// module for its type runs): a put of the same commit that is not partial replaces it.
    pub partial: bool,
}

/// One batch of changes, which adds at most one segment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Batch {
    pub put: Vec<Put>,
    /// Documents gone at a commit (a block's type no longer answers the key).
    pub removed: Vec<(DocId, Commit)>,
    /// Blocks that left the tree, every document of each.
    pub removed_blocks: Vec<(Vec<u8>, Commit)>,
    /// The sequence of changes (`changed-since`) the index has now seen every change up to.
    pub watermark: Option<u64>,
}

/// A document table entry: what the index holds for a document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub doc: DocId,
    /// The newest commit the index has seen the document at.
    pub commit: Commit,
    /// The commit whose text its postings hold: `commit`, or older when a
    /// `Put` without text moved only `commit`.
    pub indexed: Commit,
    pub source: Option<Hash>,
    /// The digest of the text its postings hold (`Text::digest`), so a put of the same text
    /// moves only its commit and source. None for a removed document.
    pub digest: Option<Hash>,
    /// None for a removed document.
    place: Option<Place>,
    /// Its text came from a partial put (`Put::partial`): a full put at the same commit
    /// replaces it, here and in a merge.
    pub partial: bool,
}

impl Entry {
    pub fn is_removed(&self) -> bool {
        self.place.is_none()
    }

    fn tombstone(doc: DocId, commit: Commit) -> Self {
        Self {
            doc,
            commit,
            indexed: commit,
            source: None,
            digest: None,
            place: None,
            partial: false,
        }
    }

    /// Whether `change` replaces this entry: it is newer, or a full put of the commit this
    /// entry holds partially.
    fn replaced_by(&self, change: &Change, latest: i64) -> bool {
        let (held, by) = (
            self.commit.believed(latest),
            change.commit().believed(latest),
        );
        held < by
            || held == by
                && self.partial
                && matches!(change, Change::Put(put) if !put.partial)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Place {
    segment: Id,
    local: u32,
}

const NAME: u8 = 0;
const BODY: u8 = 1;

/// A segment compaction replaced, kept so a merge with an index that still
/// has it does not bring it back, until `Index::forget_before` passes `at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Retired {
    id: Id,
    /// The watermark when it was retired.
    at: u64,
}

/// A block that left the tree, masking its documents at older commits,
/// those the index never saw too, until `Index::forget_before` passes `at`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RemovedBlock {
    block: Vec<u8>,
    commit: Commit,
    at: u64,
}

/// The header of an index: its segments, its document table's chunks and its
/// watermark.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Index {
    watermark: u64,
    segments: Vec<segment::Meta>,
    retired: Vec<Retired>,
    removed_blocks: Vec<RemovedBlock>,
    table: Vec<table::ChunkRef>,
}

impl Index {
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let index: Self = postcard::from_bytes(bytes).map_err(|_| Error::Corrupt("header"))?;
        for segment in &index.segments {
            segment.check()?;
        }
        if !index
            .table
            .windows(2)
            .all(|pair| pair[0].first < pair[1].first)
        {
            return Err(Error::Corrupt("table chunk list"));
        }
        Ok(index)
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

    /// Forgets retired segments and removed blocks recorded before
    /// `watermark`, once no index that predates them can still be merged in
    /// (the index block's history retention says when).
    pub fn forget_before(&mut self, watermark: u64) {
        self.retired.retain(|retired| retired.at >= watermark);
        self.removed_blocks
            .retain(|removed| removed.at >= watermark);
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

    /// The newest commit a block was removed from the index at, if it was, as believed.
    pub fn removed_at(&self, block: &[u8], latest: i64) -> Option<Commit> {
        self.block_removed_at(block, latest)
    }

    fn block_removed_at(&self, block: &[u8], latest: i64) -> Option<Commit> {
        self.removed_blocks
            .iter()
            .filter(|removed| removed.block == block)
            .map(|removed| removed.commit.believed(latest))
            .max()
    }

    /// Applies a batch: a new segment for the documents with new text, the
    /// table chunks they change and the deletion bitmaps of what they mask.
    /// Commit times past `latest` are not believed (`Commit::believed`).
    pub async fn apply(
        &mut self,
        parts: &impl Parts,
        origin: Id,
        batch: Batch,
        latest: i64,
    ) -> Result<Written, Error> {
        let mut written = Written::default();
        let mut table = table::Edit::new(&self.table);
        let mut masks: Vec<Place> = Vec::new();
        // Commits are compared as believed and stored as they are, so a reader whose clock is
        // behind never rewrites an honest time.
        for (block, commit) in batch.removed_blocks {
            if self
                .block_removed_at(&block, latest)
                .is_some_and(|known| known >= commit.believed(latest))
            {
                continue;
            }
            self.removed_blocks.retain(|removed| removed.block != block);
            self.removed_blocks.push(RemovedBlock {
                block: block.clone(),
                commit,
                at: self.watermark,
            });
            for entry in table.entries_of(parts, &block).await? {
                if entry.commit.believed(latest) < commit.believed(latest) {
                    masks.extend(entry.place);
                    table.put(Entry::tombstone(entry.doc, commit));
                }
            }
        }
        let mut newest: BTreeMap<DocId, Change> = BTreeMap::new();
        for item in batch.put {
            newest
                .entry(item.doc.clone())
                .or_insert(Change::Put(item.clone()))
                .newest(Change::Put(item), latest);
        }
        for (doc, commit) in batch.removed {
            newest
                .entry(doc.clone())
                .or_insert(Change::Remove(commit))
                .newest(Change::Remove(commit), latest);
        }
        let mut fresh = Vec::new();
        for (doc, change) in newest {
            if self
                .block_removed_at(&doc.block, latest)
                .is_some_and(|removed| removed >= change.commit().believed(latest))
            {
                continue;
            }
            let old = table.get(parts, &doc).await?.cloned();
            if old.as_ref().is_some_and(|old| !old.replaced_by(&change, latest)) {
                continue;
            }
            let old_place = old.as_ref().and_then(|old| old.place);
            match change {
                Change::Remove(commit) => {
                    masks.extend(old_place);
                    table.put(Entry::tombstone(doc, commit));
                }
                // Text the index already holds for it: only its commit and source move.
                Change::Put(Put {
                    text: Some(text),
                    commit,
                    source,
                    partial,
                    ..
                }) if old
                    .as_ref()
                    .is_some_and(|old| !old.is_removed() && old.digest == Some(text.digest())) =>
                {
                    if let Some(mut entry) = old {
                        entry.commit = commit;
                        entry.source = source;
                        entry.partial = partial;
                        table.put(entry);
                    }
                }
                Change::Put(Put {
                    text: Some(text),
                    commit,
                    source,
                    partial,
                    ..
                }) => {
                    masks.extend(old_place);
                    fresh.push((doc, commit, source, text, partial));
                }
                Change::Put(Put {
                    text: None,
                    commit,
                    source,
                    ..
                }) => match old {
                    // Only text from the same part: one put without text names the part
                    // whose text it says the index holds.
                    Some(mut entry)
                        if !entry.is_removed() && source.is_some() && entry.source == source =>
                    {
                        entry.commit = commit;
                        table.put(entry);
                    }
                    _ => written.needs_text.push(doc),
                },
            }
        }
        if !fresh.is_empty() {
            let (meta, segment_parts, places) = segment::build_from_text(
                origin,
                fresh
                    .iter()
                    .map(|(doc, commit, _, text, _)| (doc, *commit, text)),
            );
            for ((doc, commit, source, text, partial), local) in fresh.into_iter().zip(places) {
                table.put(Entry {
                    doc,
                    commit,
                    indexed: commit,
                    source,
                    digest: Some(text.digest()),
                    place: Some(Place {
                        segment: origin,
                        local,
                    }),
                    partial,
                });
            }
            written.added.extend(segment_parts);
            self.segments.push(meta);
        }
        self.mask(parts, origin, &masks, &[], &mut written).await?;
        self.table = table.finish(origin, &mut written);
        if let Some(watermark) = batch.watermark {
            self.watermark = watermark;
        }
        Ok(written)
    }

    /// Sets the deletion bits of `set` and clears those of `clear`, rewriting
    /// each bitmap that changes.
    async fn mask(
        &mut self,
        parts: &impl Parts,
        origin: Id,
        set: &[Place],
        clear: &[Place],
        written: &mut Written,
    ) -> Result<(), Error> {
        for segment in &mut self.segments {
            let of = |places: &[Place]| -> Vec<u32> {
                places
                    .iter()
                    .filter(|place| place.segment == segment.id)
                    .map(|place| place.local)
                    .collect()
            };
            let (set, clear) = (of(set), of(clear));
            if set.is_empty() && clear.is_empty() {
                continue;
            }
            let mut bits = segment.deletions(parts).await?;
            let before = bits.clone();
            segment.change_bits(&mut bits, &set, &clear);
            if bits != before {
                segment.set_deletions(origin, bits, written);
            }
        }
        Ok(())
    }
}

/// The newest change a batch makes to one document.
enum Change {
    Put(Put),
    Remove(Commit),
}

impl Change {
    fn commit(&self) -> Commit {
        match self {
            Change::Put(put) => put.commit,
            Change::Remove(commit) => *commit,
        }
    }

    /// Keeps the newer change, and of two at one commit a full put over a partial one.
    fn newest(&mut self, other: Change, latest: i64) {
        let partial = |change: &Change| matches!(change, Change::Put(put) if put.partial);
        let (held, by) = (
            self.commit().believed(latest),
            other.commit().believed(latest),
        );
        if by > held || by == held && partial(self) && !partial(&other)
        {
            *self = other;
        }
    }
}

#[cfg(test)]
mod tests;
