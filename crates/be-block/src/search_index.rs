//! The search index block (plans/search.md): a `be_lib_search::Index` kept as the block's parts.
//! The first part is the index's header with the length of every part it names, in
//! `Index::parts` order; each of those parts is a child after it, so a replica loads only the
//! header (`Loads::FirstPart`) and a query or a batch reads the rest by range.
//!
//! Hand-written rather than a `Document`, which loads whole. Two sides merge by parts
//! (`MergeParts`) as `Index::merge` unions them; within a session, where nothing can be read,
//! a side unchanged from the base gives way and anything else keeps ours as a conflict.

use std::{collections::HashMap, ops::Range};

use be_block_commit::MergeResult;
use be_lib_search::{Batch, DocId, Error, Id, Index, PartName, Parts, Written};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    BlockContent, ContentError, LiveEdit, Loads, Merge, MergeParts, Part, PartReader, PartRef,
    Stored,
};

/// A part of the index: its bytes, written here and not saved yet, or the saved part a save
/// reuses.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Held {
    Bytes(Vec<u8>),
    Saved(PartRef),
}

impl Held {
    fn length(&self) -> u64 {
        match self {
            Self::Bytes(bytes) => bytes.len() as u64,
            Self::Saved(part) => part.length(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct Header {
    index: Vec<u8>,
    /// The length of each part `Index::parts` names, in that order.
    lengths: Vec<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct SearchIndexContent {
    index: Index,
    /// Every part the header names, in `Index::parts` order.
    parts: Vec<(PartName, Held)>,
}

/// An operation: the index as a writer left it, with every part it names saved already
/// (`put-part`, guides/type_modules.md, "Parts named by hash").
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SetIndex {
    pub header: Vec<u8>,
    pub parts: Vec<PartRef>,
}

impl SearchIndexContent {
    pub fn index(&self) -> &Index {
        &self.index
    }

    /// The saved part a name is, while it is one.
    pub fn saved(&self, name: &PartName) -> Option<PartRef> {
        self.parts.iter().find_map(|(held, part)| match part {
            Held::Saved(part) if held == name => Some(*part),
            _ => None,
        })
    }

    /// Applies a batch (`Index::apply`), reading what it needs with `read`; answers the
    /// documents whose text it lacks (`Written::needs_text`).
    pub async fn apply<F: AsyncRead>(
        &mut self,
        read: &F,
        origin: Id,
        batch: Batch,
        latest: i64,
    ) -> Result<Vec<DocId>, Error> {
        let mut index = self.index.clone();
        let written = index
            .apply(&self.reader([self], read), origin, batch, latest)
            .await?;
        let needs_text = written.needs_text.clone();
        *self = Self::rebuilt(index, written, [&*self]);
        Ok(needs_text)
    }

    /// The best `limit` hits of `query` (`Index::search`).
    pub async fn search<F: AsyncRead>(
        &self,
        read: &F,
        query: &be_lib_search::Query,
        limit: usize,
        latest: i64,
    ) -> Result<Vec<be_lib_search::Hit>, Error> {
        self.index
            .search(&self.reader([self], read), query, limit, latest)
            .await
    }

    /// The table entries of a block's documents (`Index::entries_of`).
    pub async fn entries_of<F: AsyncRead>(
        &self,
        read: &F,
        block: &[u8],
    ) -> Result<Vec<be_lib_search::Entry>, Error> {
        self.index
            .entries_of(&self.reader([self], read), block)
            .await
    }

    fn reader<'a, const N: usize, F: AsyncRead>(
        &self,
        sides: [&'a Self; N],
        read: &'a F,
    ) -> Reader<'a, F> {
        let mut parts = HashMap::new();
        for side in sides {
            for (name, held) in &side.parts {
                parts.entry(*name).or_insert(held);
            }
        }
        Reader { parts, read }
    }

    /// The content of `index` after a change wrote `written`: its new parts as bytes, and
    /// every other part it names as one of `sides` held it.
    fn rebuilt<const N: usize>(index: Index, written: Written, sides: [&Self; N]) -> Self {
        let mut added: HashMap<PartName, Vec<u8>> = written.added.into_iter().collect();
        let parts = index
            .parts()
            .into_iter()
            .map(|name| {
                let held = added.remove(&name).map(Held::Bytes).or_else(|| {
                    sides.iter().find_map(|side| {
                        side.parts
                            .iter()
                            .find(|(held, _)| *held == name)
                            .map(|(_, part)| part.clone())
                    })
                });
                (name, held.unwrap_or(Held::Bytes(Vec::new())))
            })
            .collect();
        Self { index, parts }
    }

    fn header(&self) -> Vec<u8> {
        postcard::to_allocvec(&Header {
            index: self.index.encode(),
            lengths: self.parts.iter().map(|(_, part)| part.length()).collect(),
        })
        .expect("a header encodes")
    }

    fn decode_header(bytes: &[u8]) -> Result<(Index, Vec<PartName>, Vec<u64>), ContentError> {
        let (header, _): (Header, _) =
            postcard::take_from_bytes(bytes).map_err(|_| ContentError::Truncated)?;
        let index = Index::decode(&header.index).map_err(|_| ContentError::Truncated)?;
        let names = index.parts();
        if names.len() != header.lengths.len() {
            return Err(ContentError::Truncated);
        }
        Ok((index, names, header.lengths))
    }

    fn header_length(bytes: &[u8]) -> Result<usize, ContentError> {
        let (_, rest): (Header, _) =
            postcard::take_from_bytes(bytes).map_err(|_| ContentError::Truncated)?;
        Ok(bytes.len() - rest.len())
    }
}

/// Reads a range of a saved part, as the host's `read-part` or a module's `PartReader` does.
/// Boxed, as a future returned from a trait method in another crate must be
/// (guides/type_modules.md, "The generator").
pub trait AsyncRead {
    fn read(
        &self,
        part: PartRef,
        range: Range<u64>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + '_>>;
}

impl AsyncRead for PartReader {
    fn read(
        &self,
        part: PartRef,
        range: Range<u64>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<u8>, String>> + '_>> {
        let end = range.end.min(part.length());
        let start = range.start.min(end);
        PartReader::read(self, part, start, end - start)
    }
}

/// The index's parts by name, read from what a side holds or with `read`.
struct Reader<'a, F> {
    parts: HashMap<PartName, &'a Held>,
    read: &'a F,
}

impl<F: AsyncRead> Parts for Reader<'_, F> {
    fn read<'a>(
        &'a self,
        part: &'a PartName,
        range: Range<u64>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + 'a>> {
        Box::pin(async move {
            match self.parts.get(part) {
                Some(Held::Bytes(bytes)) => {
                    let end = (range.end.min(bytes.len() as u64)) as usize;
                    let start = (range.start as usize).min(end);
                    Ok(bytes[start..end].to_vec())
                }
                Some(Held::Saved(saved)) => {
                    let end = range.end.min(saved.length());
                    let start = range.start.min(end);
                    self.read
                        .read(*saved, start..end)
                        .await
                        .map_err(Error::Read)
                }
                None => Err(Error::Read(format!("the index names no part {part:?}"))),
            }
        })
    }
}

impl BlockContent for SearchIndexContent {
    const CONTENT_TYPE: Uuid = be_block_graph::SEARCH_INDEX_TYPE;

    const LOADS: Loads = Loads::FirstPart;

    /// The header and the bytes of the parts held, which is the whole stored form for content
    /// that holds every part.
    fn encode(&self) -> Vec<u8> {
        let mut bytes = self.header();
        for (_, part) in &self.parts {
            if let Held::Bytes(held) = part {
                bytes.extend_from_slice(held);
            }
        }
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        let (index, names, lengths) = Self::decode_header(bytes)?;
        let mut at = Self::header_length(bytes)?;
        let mut parts = Vec::with_capacity(names.len());
        for (name, length) in names.into_iter().zip(lengths) {
            let end = usize::try_from(length)
                .ok()
                .and_then(|length| at.checked_add(length))
                .ok_or(ContentError::Truncated)?;
            let held = bytes.get(at..end).ok_or(ContentError::Truncated)?;
            parts.push((name, Held::Bytes(held.to_vec())));
            at = end;
        }
        Ok(Self { index, parts })
    }

    fn parts(&self) -> Part<'_> {
        let mut parts = vec![Part::from(self.header())];
        parts.extend(self.parts.iter().map(|(_, part)| match part {
            Held::Bytes(bytes) => Part::from(bytes.as_slice()),
            Held::Saved(saved) => Part::Reused(*saved),
        }));
        Part::node(parts)
    }

    fn from_stored(stored: &Stored) -> Result<Self, ContentError> {
        let Some(root) = stored
            .root
            .as_ref()
            .filter(|root| !root.children.is_empty())
        else {
            return Self::decode(&stored.bytes);
        };
        let first = &root.children[0];
        let (index, names, lengths) = match stored.holds(first) {
            true => Self::decode_header(stored.bytes_of(first))?,
            false => return Err(ContentError::Truncated),
        };
        // A node leaves out empty parts, so only the others are children.
        let mut children = root.children[1..].iter();
        let mut parts = Vec::with_capacity(names.len());
        for (name, length) in names.into_iter().zip(lengths) {
            if length == 0 {
                parts.push((name, Held::Bytes(Vec::new())));
                continue;
            }
            let child = children.next().ok_or(ContentError::Truncated)?;
            if child.reference.length() != length {
                return Err(ContentError::Truncated);
            }
            parts.push((
                name,
                match stored.holds(child) {
                    true => Held::Bytes(stored.bytes_of(child).to_vec()),
                    false => Held::Saved(child.reference),
                },
            ));
        }
        Ok(Self { index, parts })
    }
}

impl LiveEdit for SearchIndexContent {
    type Op = SetIndex;

    /// An index whose header or parts do not agree is ignored.
    fn apply(&mut self, operation: &SetIndex) {
        let Ok((index, names, lengths)) = Self::decode_header(&operation.header) else {
            return;
        };
        let mut saved = operation.parts.iter();
        let mut parts = Vec::with_capacity(names.len());
        for (name, length) in names.into_iter().zip(lengths) {
            if length == 0 {
                parts.push((name, Held::Bytes(Vec::new())));
                continue;
            }
            match saved.next() {
                Some(part) if part.length() == length => parts.push((name, Held::Saved(*part))),
                _ => return,
            }
        }
        *self = Self { index, parts };
    }
}

impl Merge for SearchIndexContent {
    /// Over what each side holds, which cannot be read here: a side whose header is the base's
    /// gives way, and two changed sides keep ours as a conflict, which `merge_parts` unions.
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self> {
        let header = Self::header;
        if header(ours) == header(base) || header(ours) == header(theirs) {
            return MergeResult::Clean(theirs.clone());
        }
        if header(theirs) == header(base) {
            return MergeResult::Clean(ours.clone());
        }
        MergeResult::Conflicted {
            value: ours.clone(),
            conflicts: 1,
        }
    }
}

impl MergeParts for SearchIndexContent {
    /// The union of the two sides (`Index::merge`), which never conflicts. Its new parts are
    /// named under an id taken from both headers, so every device merging the same sides
    /// writes the same parts.
    fn merge_parts(
        _: Self,
        ours: Self,
        theirs: Self,
        parts: PartReader,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<MergeResult<Self>, String>>>> {
        Box::pin(async move {
            let mut origin = [0; 16];
            let both = be_block_store::Hash::of_parts(&[&ours.header(), &theirs.header()]);
            origin.copy_from_slice(&both.as_bytes()[..16]);
            let reader = ours.reader([&ours, &theirs], &parts);
            let (index, written) = ours
                .index
                .merge(&theirs.index, &reader, origin)
                .await
                .map_err(|error| error.to_string())?;
            Ok(MergeResult::Clean(Self::rebuilt(
                index,
                written,
                [&ours, &theirs],
            )))
        })
    }
}
