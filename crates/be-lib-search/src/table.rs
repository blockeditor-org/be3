//! The document table: entries sorted by document, in chunks of at most
//! `CHUNK` entries, each a part named in the header with its first document.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{DocId, Entry, Error, Id, PartKind, PartName, Parts, Written};

const CHUNK: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChunkRef {
    pub(crate) first: DocId,
    pub(crate) part: PartName,
}

pub(crate) async fn read_chunk(parts: &impl Parts, chunk: &ChunkRef) -> Result<Vec<Entry>, Error> {
    let bytes = parts.read(&chunk.part, 0..u64::MAX).await?;
    postcard::from_bytes(&bytes).map_err(|_| Error::Corrupt("table chunk"))
}

/// Writes `entries` (sorted) as new chunks named under `origin`, numbering
/// them from `*next`.
pub(crate) fn write_chunks(
    entries: &[Entry],
    origin: Id,
    next: &mut u32,
    written: &mut Written,
) -> Vec<ChunkRef> {
    entries
        .chunks(CHUNK)
        .map(|chunk| {
            let part = PartName {
                origin,
                kind: PartKind::Table(*next),
            };
            *next += 1;
            written.added.push((
                part,
                postcard::to_allocvec(chunk).expect("a table chunk encodes"),
            ));
            ChunkRef {
                first: chunk[0].doc.clone(),
                part,
            }
        })
        .collect()
}

/// Reads and changes the table a chunk at a time; `finish` writes the
/// chunks it changed.
pub(crate) struct Edit {
    dir: Vec<ChunkRef>,
    loaded: BTreeMap<usize, Vec<Entry>>,
    changed: BTreeSet<usize>,
}

impl Edit {
    pub(crate) fn new(dir: &[ChunkRef]) -> Self {
        Self {
            dir: dir.to_vec(),
            loaded: BTreeMap::new(),
            changed: BTreeSet::new(),
        }
    }

    fn chunk_of(&self, doc: &DocId) -> usize {
        self.dir
            .partition_point(|chunk| chunk.first <= *doc)
            .saturating_sub(1)
    }

    async fn load(&mut self, parts: &impl Parts, index: usize) -> Result<&mut Vec<Entry>, Error> {
        if !self.loaded.contains_key(&index) {
            let entries = match self.dir.get(index) {
                Some(chunk) => read_chunk(parts, chunk).await?,
                None => Vec::new(),
            };
            self.loaded.insert(index, entries);
        }
        Ok(self.loaded.get_mut(&index).unwrap())
    }

    pub(crate) async fn get(
        &mut self,
        parts: &impl Parts,
        doc: &DocId,
    ) -> Result<Option<&Entry>, Error> {
        let entries = self.load(parts, self.chunk_of(doc)).await?;
        Ok(match entries.binary_search_by(|entry| entry.doc.cmp(doc)) {
            Ok(at) => Some(&entries[at]),
            Err(_) => None,
        })
    }

    /// Inserts or replaces the entry of its document; its chunk must have
    /// been read (by `get` or `entries_of`).
    pub(crate) fn put(&mut self, entry: Entry) {
        let index = self.chunk_of(&entry.doc);
        let entries = self.loaded.get_mut(&index).expect("put after get");
        match entries.binary_search_by(|other| other.doc.cmp(&entry.doc)) {
            Ok(at) => entries[at] = entry,
            Err(at) => entries.insert(at, entry),
        }
        self.changed.insert(index);
    }

    pub(crate) async fn entries_of(
        &mut self,
        parts: &impl Parts,
        block: &[u8],
    ) -> Result<Vec<Entry>, Error> {
        let start = self.chunk_of(&DocId {
            block: block.to_vec(),
            key: String::new(),
        });
        let end = self
            .dir
            .partition_point(|chunk| chunk.first.block.as_slice() <= block)
            .max(start + 1);
        let mut out = Vec::new();
        for index in start..end {
            let entries = self.load(parts, index).await?;
            out.extend(
                entries
                    .iter()
                    .filter(|entry| entry.doc.block == block)
                    .cloned(),
            );
        }
        Ok(out)
    }

    /// The new chunk list, writing the changed chunks under `origin`.
    pub(crate) fn finish(self, origin: Id, written: &mut Written) -> Vec<ChunkRef> {
        let count = self.dir.len().max(1);
        let mut next = 0;
        let mut dir = Vec::new();
        for index in 0..count {
            if !self.changed.contains(&index) {
                dir.extend(self.dir.get(index).cloned());
                continue;
            }
            if let Some(old) = self.dir.get(index) {
                written.dropped.push(old.part);
            }
            dir.extend(write_chunks(
                &self.loaded[&index],
                origin,
                &mut next,
                written,
            ));
        }
        dir
    }
}
