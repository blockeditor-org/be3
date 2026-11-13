//! Tiered compaction: segments of a similar number of live documents (the
//! same power of `FACTOR`) merge once there are `FACTOR` of them, so the count
//! stays logarithmic in the documents and each document is rewritten a
//! logarithmic number of times. A segment more than half masked is rewritten
//! alone, and one wholly masked is dropped.

use std::collections::{BTreeMap, HashMap};

use crate::{
    Error, Id, Index, Parts, Place, Retired, Written,
    segment::{self, Meta, Posting},
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
    /// Leaves out parts both added and dropped by the same change.
    pub(crate) fn tidy(&mut self) {
        let dropped: std::collections::HashSet<_> =
            std::mem::take(&mut self.dropped).into_iter().collect();
        let added: std::collections::HashSet<_> =
            self.added.iter().map(|(name, _)| *name).collect();
        self.added.retain(|(name, _)| !dropped.contains(name));
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
    /// left for later. What it retires is recorded at `now` (milliseconds), for
    /// `Index::forget_before`.
    pub async fn compact(
        &mut self,
        parts: &impl Parts,
        origin: Id,
        now: i64,
        most: usize,
        bytes: u64,
        length: impl Fn(&crate::PartName) -> u64,
    ) -> Result<Written, Error> {
        let mut written = Written::default();
        let mut table = table::Edit::new(&self.table);
        let mut built: HashMap<Id, segment::Named> = HashMap::new();
        let mut n = 0;
        loop {
            let mut index = 0;
            while index < self.segments.len() {
                if self.segments[index].live() == 0 {
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
            // A segment more than half masked first, then a tier of `FACTOR`; the first whose
            // inputs fit under `bytes`.
            let mut candidates: Vec<Vec<usize>> = self
                .segments
                .iter()
                .enumerate()
                .filter(|(_, meta)| u64::from(meta.deleted) * 2 > u64::from(meta.docs))
                .map(|(index, _)| vec![index])
                .collect();
            let mut tiers: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
            for (index, meta) in self.segments.iter().enumerate() {
                tiers.entry(tier(meta.live())).or_default().push(index);
            }
            candidates.extend(
                tiers
                    .into_values()
                    .filter(|group| group.len() >= FACTOR)
                    .map(|mut group| {
                        group.sort_by_key(|&index| self.segments[index].live());
                        group.truncate(FACTOR);
                        group
                    }),
            );
            let size = |group: &Vec<usize>| -> u64 {
                group
                    .iter()
                    .flat_map(|&index| self.segments[index].parts())
                    .map(|part| length(&part))
                    .sum()
            };
            let Some(group) = candidates.into_iter().find(|group| size(group) <= bytes) else {
                break;
            };
            n += 1;
            let id = derive(origin, n);
            let inputs: Vec<Meta> = group
                .iter()
                .map(|&index| self.segments[index].clone())
                .collect();
            let source = Overlay {
                parts,
                built: &built,
            };
            let (meta, segment_parts) = merge_segments(&source, &inputs, id, &mut table).await?;
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
            built.insert(id, segment_parts.clone());
            written.added.extend(segment_parts);
            self.segments.push(meta);
        }
        self.table = table.finish(origin, &mut written);
        written.tidy();
        Ok(written)
    }
}

/// The caller's parts and the segments this compaction built so far.
struct Overlay<'a, P> {
    parts: &'a P,
    built: &'a HashMap<Id, segment::Named>,
}

impl<P: Parts> Parts for Overlay<'_, P> {
    fn read<'a>(
        &'a self,
        part: &'a crate::PartName,
        range: std::ops::Range<u64>,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + 'a>> {
        Box::pin(async move {
            if let Some(segment) = self.built.get(&part.origin)
                && let Some((_, bytes)) = segment.iter().find(|(name, _)| name == part)
            {
                let end = range.end.min(bytes.len() as u64) as usize;
                return Ok(bytes[(range.start as usize).min(end)..end].to_vec());
            }
            self.parts.read(part, range).await
        })
    }
}

/// One segment of the live documents of `inputs` (those the table points
/// to), moving their table entries to it.
async fn merge_segments(
    parts: &impl Parts,
    inputs: &[Meta],
    id: Id,
    table: &mut table::Edit,
) -> Result<(Meta, segment::Named), Error> {
    let mut documents = Vec::new();
    let mut from: Vec<(usize, u32)> = Vec::new();
    for (input, meta) in inputs.iter().enumerate() {
        let deletions = meta.deletions(parts).await?;
        for (local, document) in meta.documents(parts).await?.into_iter().enumerate() {
            let local = local as u32;
            if deletions.get(local) {
                continue;
            }
            let here = Place {
                segment: meta.id,
                local,
            };
            if table
                .get(parts, &document.doc)
                .await?
                .is_none_or(|entry| entry.place != Some(here))
            {
                continue;
            }
            documents.push(document);
            from.push((input, local));
        }
    }
    let locals = segment::assign(&documents);
    let remap: HashMap<(usize, u32), u32> =
        from.iter().copied().zip(locals.iter().copied()).collect();
    let mut terms: BTreeMap<Vec<u8>, Vec<Posting>> = BTreeMap::new();
    for (input, meta) in inputs.iter().enumerate() {
        for (term, list) in meta.read_all(parts).await? {
            let moved: Vec<Posting> = list
                .into_iter()
                .filter_map(|mut posting| {
                    posting.doc = *remap.get(&(input, posting.doc))?;
                    Some(posting)
                })
                .collect();
            if !moved.is_empty() {
                terms.entry(term).or_default().extend(moved);
            }
        }
    }
    for list in terms.values_mut() {
        list.sort_by_key(|posting| posting.doc);
    }
    for (document, &local) in documents.iter().zip(&locals) {
        if let Some(mut entry) = table.get(parts, &document.doc).await?.cloned() {
            entry.place = Some(Place { segment: id, local });
            table.put(entry);
        }
    }
    let documents = segment::sorted(documents, &locals);
    Ok(segment::build(id, &documents, &terms))
}
