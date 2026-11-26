//! Reading a segment forward, as a merge does: its documents in local order and its terms in
//! dictionary order, each part through a `Window`, so the merge holds a window of each input
//! part rather than the part.

use std::collections::VecDeque;

use crate::{
    Error, PartKind, PartName, Parts,
    segment::{self, Document, Meta, Posting, TermInfo},
};

/// The bytes a `Window` reads at once: about a sealed chunk's (`ChunkerConfig::DEFAULT`), as
/// a read of a saved part opens every chunk the range touches. Small in this crate's tests, so
/// they cross many windows.
const WINDOW: u64 = if cfg!(test) { 2 << 10 } else { 1 << 20 };

/// One part read forward a window at a time: a range past the window reads the next window
/// from its start, or just the range when it is longer than one.
pub(crate) struct Window {
    name: PartName,
    from: u64,
    bytes: Vec<u8>,
}

impl Window {
    pub(crate) fn new(name: PartName) -> Self {
        Self {
            name,
            from: 0,
            bytes: Vec::new(),
        }
    }

    /// `len` bytes from `start`, or Corrupt past the part's end.
    pub(crate) async fn read(
        &mut self,
        parts: &impl Parts,
        start: u64,
        len: u64,
    ) -> Result<&[u8], Error> {
        let end = segment::add(start, len)?;
        if start < self.from || end > self.from + self.bytes.len() as u64 {
            self.bytes = parts
                .read(&self.name, start..end.max(start.saturating_add(WINDOW)))
                .await?;
            self.from = start;
        }
        segment::slice(&self.bytes, start - self.from, len)
    }
}

/// A segment's documents, read in local order.
pub(crate) struct DocumentReader {
    docs: u32,
    offsets: Window,
    records: Window,
}

impl DocumentReader {
    pub(crate) async fn open(parts: &impl Parts, meta: &Meta) -> Result<Self, Error> {
        let name = meta.part(PartKind::Documents);
        let mut offsets = Window::new(name);
        if segment::le(offsets.read(parts, 0, 4).await?, 0)? != meta.docs {
            return Err(Error::Corrupt("documents"));
        }
        Ok(Self {
            docs: meta.docs,
            offsets,
            records: Window::new(name),
        })
    }

    /// The document at `local`, which comes after the last one read.
    pub(crate) async fn get(&mut self, parts: &impl Parts, local: u32) -> Result<Document, Error> {
        if local >= self.docs {
            return Err(Error::Corrupt("document id"));
        }
        let pair = self.offsets.read(parts, 4 + 4 * u64::from(local), 8).await?;
        let (start, end) = (segment::le(pair, 0)?, segment::le(pair, 4)?);
        if start > end {
            return Err(Error::Corrupt("documents"));
        }
        let base = 4 + 4 * (u64::from(self.docs) + 1);
        let record = self
            .records
            .read(parts, base + u64::from(start), u64::from(end - start))
            .await?;
        postcard::from_bytes(record).map_err(|_| Error::Corrupt("document"))
    }
}

/// A segment's terms with their postings and positions, read in dictionary order.
pub(crate) struct TermReader<'a> {
    meta: &'a Meta,
    /// The next dictionary block to read.
    block: usize,
    queue: VecDeque<(Vec<u8>, TermInfo)>,
    dictionary: Window,
    postings: Window,
    positions: Window,
}

impl<'a> TermReader<'a> {
    pub(crate) fn new(meta: &'a Meta) -> Self {
        Self {
            meta,
            block: 0,
            queue: VecDeque::new(),
            dictionary: Window::new(meta.part(PartKind::Dictionary)),
            postings: Window::new(meta.part(PartKind::Postings)),
            positions: Window::new(meta.part(PartKind::Positions)),
        }
    }

    /// The next term, reading the next dictionary block when the last is done; None after the
    /// last term.
    pub(crate) async fn peek(&mut self, parts: &impl Parts) -> Result<Option<&[u8]>, Error> {
        while self.queue.is_empty() && self.block < self.meta.firsts.len() {
            let (start, end) = (
                self.meta.blocks[self.block],
                self.meta.blocks[self.block + 1],
            );
            let bytes = self
                .dictionary
                .read(parts, u64::from(start), u64::from(end - start))
                .await?;
            self.queue = segment::decode_dictionary_block(bytes)?.into();
            self.block += 1;
        }
        Ok(self.queue.front().map(|(term, _)| term.as_slice()))
    }

    /// The next term's postings with their positions, moving past it.
    pub(crate) async fn take(&mut self, parts: &impl Parts) -> Result<Vec<Posting>, Error> {
        let Some((_, info)) = self.queue.pop_front() else {
            return Err(Error::Corrupt("dictionary"));
        };
        let bytes = self
            .postings
            .read(parts, info.postings, u64::from(info.postings_len))
            .await?;
        let skip = segment::decode_skip(segment::slice(bytes, 0, u64::from(info.skip_len))?, info.df)?;
        let mut list = Vec::new();
        let mut at = u64::from(info.skip_len);
        let mut previous = 0;
        for (index, entry) in skip.iter().enumerate() {
            let count = segment::block_count(info.df, index);
            list.push(segment::decode_block(
                segment::slice(bytes, at, u64::from(entry.len))?,
                count,
                previous,
            )?);
            at = segment::add(at, u64::from(entry.len))?;
            previous = entry.last;
        }
        let total = skip.iter().try_fold(0u64, |sum, entry| {
            segment::add(sum, u64::from(entry.positions_len))
        })?;
        let positions = self.positions.read(parts, info.positions, total).await?;
        let mut at = 0;
        for (block, entry) in list.iter_mut().zip(&skip) {
            segment::decode_positions(
                segment::slice(positions, at, u64::from(entry.positions_len))?,
                block,
            )?;
            at += u64::from(entry.positions_len);
        }
        Ok(list.into_iter().flatten().collect())
    }
}
