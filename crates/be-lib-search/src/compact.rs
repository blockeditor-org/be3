//! Tiered compaction: segments of a similar number of live documents (the
//! same power of `FACTOR`) merge once there are `FACTOR` of them, so the count
//! stays logarithmic in the documents and each document is rewritten a
//! logarithmic number of times. A segment more than half masked is rewritten
//! alone, and one wholly masked is dropped.

use std::collections::{BTreeMap, HashSet};

use crate::{
    Entry, Error, Id, Index, PartKind, Parts, PartsOut, Place, Retired, Written,
    scan::{DocumentReader, TermReader},
    segment::{self, Document, Meta, Posting, TermWriter},
    table,
};

const FACTOR: usize = 4;

fn tier(live: u32) -> u32 {
    let mut tier = 0;
    let mut size = live.max(1) as u64;
    while size >= FACTOR as u64 {
        size /= FACTOR as u64;
        tier += 1;
    }
    tier
}

/// A distinct id for the `n`th segment a compaction or merge under `origin`
/// writes.
pub(crate) fn derive(origin: Id, n: u32) -> Id {
    let mut id = origin;
    for (byte, n) in id[12..].iter_mut().zip(n.to_le_bytes()) {
        *byte ^= n;
    }
    id
}

impl Written {
    /// Leaves out parts both added (or saved) and dropped by the same change.
    pub(crate) fn tidy(&mut self) {
        let dropped: HashSet<_> = std::mem::take(&mut self.dropped).into_iter().collect();
        let added: HashSet<_> = self
            .added
            .iter()
            .map(|(name, _)| *name)
            .chain(self.saved.iter().copied())
            .collect();
        self.added.retain(|(name, _)| !dropped.contains(name));
        self.saved.retain(|name| !dropped.contains(name));
        self.dropped = dropped
            .into_iter()
            .filter(|name| !added.contains(name))
            .collect();
        self.dropped.sort();
        self.dropped.dedup();
    }
}

impl Index {
    /// Merges segments until no tier holds `FACTOR` of them, or `most` merges are done: a
    /// writer compacts a few at a time with each batch, so no one write rewrites the index.
    /// A merge whose inputs' parts come to more than `bytes` (as `length` gives a part's) is
    /// not made: those segments are left as they are. Each merged segment's parts are written
    /// through `out` (`Written::saved`); the table and deletion bitmaps come back as bytes.
    /// What it retires is recorded at `now` (milliseconds), for `Index::forget_before`.
    #[allow(clippy::too_many_arguments)]
    pub async fn compact(
        &mut self,
        parts: &impl Parts,
        out: &impl PartsOut,
        origin: Id,
        now: i64,
        most: usize,
        bytes: u64,
        length: impl Fn(&crate::PartName) -> u64,
    ) -> Result<Written, Error> {
        let mut written = Written::default();
        let mut table = table::Edit::new(&self.table);
        // The segments this pass wrote, which it leaves for the next: a merge taking one would
        // drop parts already sealed through `out`, and nothing would remove them.
        let mut merged: HashSet<Id> = HashSet::new();
        let mut n = 0;
        loop {
            let mut index = 0;
            while index < self.segments.len() {
                let meta = &self.segments[index];
                if meta.live() == 0 && !merged.contains(&meta.id) {
                    let meta = self.segments.remove(index);
                    written.dropped.extend(meta.parts());
                    self.retired.push(Retired {
                        id: meta.id,
                        at: now,
                    });
                } else {
                    index += 1;
                }
            }
            if n as usize >= most {
                break;
            }
            let Some(group) = self
                .merges(&merged)
                .into_iter()
                .find(|group| self.merge_bytes(group, &length) <= bytes)
            else {
                break;
            };
            n += 1;
            let id = derive(origin, n);
            let inputs: Vec<Meta> = group
                .iter()
                .map(|&index| self.segments[index].clone())
                .collect();
            let meta = merge_segments(parts, out, &inputs, id, &mut table).await?;
            let mut group = group;
            group.sort_unstable_by(|a, b| b.cmp(a));
            for index in group {
                let meta = self.segments.remove(index);
                written.dropped.extend(meta.parts());
                self.retired.push(Retired {
                    id: meta.id,
                    at: now,
                });
            }
            written.saved.extend(meta.parts());
            merged.insert(id);
            self.segments.push(meta);
        }
        self.table = table.finish(origin, &mut written);
        written.tidy();
        Ok(written)
    }
}

impl Index {
    /// Whether a merge compaction would make reads more than `bytes` of parts (as `length`
    /// gives a part's): what a capped compaction leaves, for a pass without the cap.
    pub fn merges_over(&self, bytes: u64, length: impl Fn(&crate::PartName) -> u64) -> bool {
        self.merges(&HashSet::new())
            .iter()
            .any(|group| self.merge_bytes(group, &length) > bytes)
    }

    /// The merges compaction would make, by the segments' places, in the order it tries them:
    /// a segment more than half masked first, then a tier of `FACTOR`. None takes a segment
    /// in `left`.
    fn merges(&self, left: &HashSet<Id>) -> Vec<Vec<usize>> {
        let segments = || {
            self.segments
                .iter()
                .enumerate()
                .filter(|(_, meta)| !left.contains(&meta.id))
        };
        let mut merges: Vec<Vec<usize>> = segments()
            .filter(|(_, meta)| u64::from(meta.deleted) * 2 > u64::from(meta.docs))
            .map(|(index, _)| vec![index])
            .collect();
        let mut tiers: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
        for (index, meta) in segments() {
            tiers.entry(tier(meta.live())).or_default().push(index);
        }
        merges.extend(
            tiers
                .into_values()
                .filter(|group| group.len() >= FACTOR)
                .map(|mut group| {
                    group.sort_by_key(|&index| self.segments[index].live());
                    group.truncate(FACTOR);
                    group
                }),
        );
        merges
    }

    /// The bytes of parts a merge of `group` reads.
    fn merge_bytes(&self, group: &[usize], length: impl Fn(&crate::PartName) -> u64) -> u64 {
        group
            .iter()
            .flat_map(|&index| self.segments[index].parts())
            .map(|part| length(&part))
            .fold(0u64, u64::saturating_add)
    }
}

/// A local id `merge_segments` drops.
const GONE: u32 = u32::MAX;

/// One segment of the live documents of `inputs` (those the table points
/// to), written through `out` as it is read, moving their table entries to it.
///
/// Each input's documents are in local order (newest first, then by document), so a k-way
/// merge of them gives the new segment's order, and of their dictionaries its terms. The
/// documents are read twice: once for the new local ids and record offsets, once to write the
/// records after them. What it holds: two `u32` per document (its new local id and its record's
/// offset), each input's deletion bitmap, one term's whole posting list with its positions, at
/// most five read windows per input (`scan::Window`, about 1 MiB each), and the table chunks
/// the moved documents' entries live in (`table::Edit`, which loads each chunk it changes). It
/// writes what `segment::build` writes from the same documents and terms.
async fn merge_segments(
    parts: &impl Parts,
    out: &impl PartsOut,
    inputs: &[Meta],
    id: Id,
    table: &mut table::Edit,
) -> Result<Meta, Error> {
    let name = |kind| crate::PartName { origin: id, kind };
    let mut documents = Vec::with_capacity(inputs.len());
    let mut deletions = Vec::with_capacity(inputs.len());
    for meta in inputs {
        documents.push(DocumentReader::open(parts, meta).await?);
        deletions.push(meta.deletions(parts).await?);
    }
    // Each input's local ids to the new ones, GONE for those dropped.
    let mut maps: Vec<Vec<u32>> = vec![Vec::new(); inputs.len()];
    // The next live document of each input, with its table entry.
    let mut heads: Vec<Option<(Document, Entry)>> = Vec::with_capacity(inputs.len());
    for input in 0..inputs.len() {
        heads.push(
            next_live(
                parts,
                &inputs[input],
                &mut documents[input],
                &deletions[input],
                &mut maps[input],
                table,
            )
            .await?,
        );
    }
    let order = |document: &Document| (std::cmp::Reverse(document.commit), document.doc.clone());
    let mut offsets = vec![0u32];
    let mut lengths = [0u64; 2];
    let mut previous = None;
    while let Some(input) = (0..inputs.len())
        .filter(|&input| heads[input].is_some())
        .min_by_key(|&input| order(&heads[input].as_ref().unwrap().0))
    {
        let (document, mut entry) = heads[input].take().unwrap();
        let key = order(&document);
        if previous.as_ref().is_some_and(|previous| *previous >= key) {
            return Err(Error::Corrupt("documents order"));
        }
        previous = Some(key);
        let local = offsets.len() as u32 - 1;
        maps[input].push(local);
        entry.place = Some(Place { segment: id, local });
        table.put(entry);
        let end = offsets[offsets.len() - 1]
            .checked_add(segment::record(&document).len() as u32)
            .ok_or(Error::Corrupt("documents"))?;
        offsets.push(end);
        lengths[0] += u64::from(document.lengths[0]);
        lengths[1] += u64::from(document.lengths[1]);
        heads[input] = next_live(
            parts,
            &inputs[input],
            &mut documents[input],
            &deletions[input],
            &mut maps[input],
            table,
        )
        .await?;
    }
    let count = offsets.len() as u32 - 1;

    // The documents part: the count, the offsets, then the records again in the new order.
    let part = name(PartKind::Documents);
    out.write(&part, &count.to_le_bytes())?;
    for window in offsets.chunks(1024) {
        let bytes: Vec<u8> = window
            .iter()
            .flat_map(|offset| offset.to_le_bytes())
            .collect();
        out.write(&part, &bytes)?;
    }
    drop(documents);
    let mut documents = Vec::with_capacity(inputs.len());
    for meta in inputs {
        documents.push(DocumentReader::open(parts, meta).await?);
    }
    let mut next = vec![0usize; inputs.len()];
    for local in 0..count {
        let input = (0..inputs.len())
            .find(|&input| {
                while maps[input].get(next[input]) == Some(&GONE) {
                    next[input] += 1;
                }
                maps[input].get(next[input]) == Some(&local)
            })
            .expect("every new local id is some input's");
        let document = documents[input].get(parts, next[input] as u32).await?;
        next[input] += 1;
        let record = segment::record(&document);
        if record.len() as u64 != u64::from(offsets[local as usize + 1] - offsets[local as usize]) {
            return Err(Error::Corrupt("documents"));
        }
        out.write(&part, &record)?;
    }
    out.finish(&part)?;
    drop(documents);

    // The terms: each input's dictionary merged in order, a term's postings at a time.
    let mut readers: Vec<TermReader> = inputs.iter().map(TermReader::new).collect();
    let mut writer = TermWriter::new();
    let mut previous: Option<Vec<u8>> = None;
    loop {
        let mut term: Option<Vec<u8>> = None;
        for reader in &mut readers {
            if let Some(next) = reader.peek(parts).await?
                && term.as_ref().is_none_or(|term| next < term.as_slice())
            {
                term = Some(next.to_vec());
            }
        }
        let Some(term) = term else {
            break;
        };
        if previous.as_ref().is_some_and(|previous| *previous >= term) {
            return Err(Error::Corrupt("dictionary order"));
        }
        let mut list: Vec<Posting> = Vec::new();
        for (input, reader) in readers.iter_mut().enumerate() {
            if reader.peek(parts).await? != Some(term.as_slice()) {
                continue;
            }
            list.extend(
                reader
                    .take(parts)
                    .await?
                    .into_iter()
                    .filter_map(|mut posting| {
                        posting.doc = *maps[input]
                            .get(posting.doc as usize)
                            .filter(|&&local| local != GONE)?;
                        Some(posting)
                    }),
            );
        }
        if !list.is_empty() {
            list.sort_by_key(|posting| posting.doc);
            writer.add(&term, &list, &mut |kind, bytes| {
                out.write(&name(kind), bytes)
            })?;
        }
        previous = Some(term);
    }
    for kind in [
        PartKind::Dictionary,
        PartKind::Postings,
        PartKind::Positions,
    ] {
        out.finish(&name(kind))?;
    }
    Ok(writer.finish(id, count, lengths))
}

/// The next document of an input the table points to, noting those before it as dropped.
async fn next_live(
    parts: &impl Parts,
    meta: &Meta,
    documents: &mut DocumentReader,
    deletions: &segment::Bits,
    map: &mut Vec<u32>,
    table: &mut table::Edit,
) -> Result<Option<(Document, Entry)>, Error> {
    while (map.len() as u64) < u64::from(meta.docs) {
        let local = map.len() as u32;
        if !deletions.get(local) {
            let document = documents.get(parts, local).await?;
            let here = Place {
                segment: meta.id,
                local,
            };
            if let Some(entry) = table.get(parts, &document.doc).await?
                && entry.place == Some(here)
            {
                let entry = entry.clone();
                return Ok(Some((document, entry)));
            }
        }
        map.push(GONE);
    }
    Ok(None)
}
