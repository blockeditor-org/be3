//! Merging two indexes, as two devices' offline batches meet: the union of
//! their segments by id (less those either retired), the newest entry per
//! document, the lower watermark. The loser of each document is masked and
//! the winner unmasked, so it never conflicts.
//!
//! A winner may point into a segment the other side compacted away (its
//! newer commit moved only the commit, keeping the old place). The loser then
//! holds the same text at its surviving place when it was indexed at the same
//! commit, and the winner takes that place; otherwise the merge has no
//! postings for its text and returns it in `Written::needs_text`.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::{
    DocId, Entry, Error, Id, Index, Parts, Place, RemovedBlock, Retired, Written,
    segment::Meta,
    table::{self, ChunkRef},
};

impl Index {
    /// The union of `self` and `other`, whose parts `parts` reads alike,
    /// writing what changed under `origin`.
    pub async fn merge(&self, other: &Index, parts: &impl Parts, origin: Id) -> Result<(Index, Written), Error> {
        let mut written = Written::default();
        let mut retired: BTreeMap<Id, u64> = BTreeMap::new();
        for Retired { id, at } in self.retired.iter().chain(&other.retired) {
            let kept = retired.entry(*id).or_insert(*at);
            *kept = (*kept).max(*at);
        }
        let mut removed_blocks: BTreeMap<Vec<u8>, RemovedBlock> = BTreeMap::new();
        for removed in self.removed_blocks.iter().chain(&other.removed_blocks) {
            match removed_blocks.get(&removed.block) {
                Some(kept) if kept.commit >= removed.commit => {}
                _ => {
                    removed_blocks.insert(removed.block.clone(), removed.clone());
                }
            }
        }
        let mut segments: Vec<(Meta, Option<Meta>)> = Vec::new();
        for meta in self.segments.iter().chain(&other.segments) {
            if retired.contains_key(&meta.id) {
                continue;
            }
            match segments.iter_mut().find(|(first, _)| first.id == meta.id) {
                Some((first, second)) => {
                    if first.deletions != meta.deletions {
                        *second = Some(meta.clone());
                    }
                }
                None => segments.push((meta.clone(), None)),
            }
        }
        let surviving: HashSet<Id> = segments.iter().map(|(meta, _)| meta.id).collect();
        let survives = |place: &Option<Place>| place.is_some_and(|place| surviving.contains(&place.segment));

        // The table: chunks both name are the same; the rest merge by entry.
        let ours: HashSet<_> = self.table.iter().map(|chunk| chunk.part).collect();
        let theirs: HashSet<_> = other.table.iter().map(|chunk| chunk.part).collect();
        let shared: Vec<ChunkRef> = self
            .table
            .iter()
            .filter(|chunk| theirs.contains(&chunk.part))
            .cloned()
            .collect();
        let unshared = self
            .table
            .iter()
            .filter(|chunk| !theirs.contains(&chunk.part))
            .chain(other.table.iter().filter(|chunk| !ours.contains(&chunk.part)));
        let chunks = crate::join::try_all(unshared.map(|chunk| table::read_chunk(parts, chunk))).await?;
        let mut sides: BTreeMap<DocId, Vec<Entry>> = BTreeMap::new();
        for entry in chunks.into_iter().flatten() {
            sides.entry(entry.doc.clone()).or_default().push(entry);
        }
        let mut masks: Vec<Place> = Vec::new();
        let mut keeps: Vec<Place> = Vec::new();
        let mut merged: Vec<Entry> = Vec::with_capacity(sides.len());
        for (doc, mut entries) in sides {
            let rank = |entry: &Entry| (entry.commit, survives(&entry.place), std::cmp::Reverse(entry.place));
            entries.sort_by_key(|entry| std::cmp::Reverse(rank(entry)));
            let newest = entries[0].clone();
            let mut winner = newest.clone();
            if winner.place.is_some() && !survives(&winner.place) {
                winner.place = entries[1..]
                    .iter()
                    .find(|other| other.indexed == winner.indexed && survives(&other.place))
                    .and_then(|other| other.place);
            }
            if let Some(removed) = removed_blocks.get(&doc.block)
                && removed.commit > winner.commit
            {
                winner = Entry::tombstone(doc.clone(), removed.commit);
            }
            for entry in &entries {
                if survives(&entry.place) && entry.place != winner.place {
                    masks.extend(entry.place);
                }
            }
            if winner.place.is_none() && !newest.is_removed() && winner.commit == newest.commit {
                // Its text is in no segment that survives.
                written.needs_text.push(doc);
                continue;
            }
            keeps.extend(winner.place);
            merged.push(winner);
        }
        let mut dir = Vec::new();
        let mut next = 0;
        let mut entries = merged.into_iter().peekable();
        for chunk in shared {
            let pending: Vec<Entry> = std::iter::from_fn(|| entries.next_if(|entry| entry.doc < chunk.first)).collect();
            dir.extend(table::write_chunks(&pending, origin, &mut next, &mut written));
            dir.push(chunk);
        }
        let pending: Vec<Entry> = entries.collect();
        dir.extend(table::write_chunks(&pending, origin, &mut next, &mut written));

        // Deletion bitmaps: both sides' bits, the losers' set, the winners' clear.
        let mut by_segment: HashMap<Id, (Vec<u32>, Vec<u32>)> = HashMap::new();
        for place in &masks {
            by_segment.entry(place.segment).or_default().0.push(place.local);
        }
        for place in &keeps {
            by_segment.entry(place.segment).or_default().1.push(place.local);
        }
        let mut metas = Vec::with_capacity(segments.len());
        for (mut meta, second) in segments {
            let (set, clear) = by_segment.remove(&meta.id).unwrap_or_default();
            if second.is_some() || !set.is_empty() || !clear.is_empty() {
                let mut bits = meta.deletions(parts).await?;
                let before = bits.clone();
                if let Some(second) = &second {
                    bits.union(&second.deletions(parts).await?);
                }
                meta.change_bits(&mut bits, &set, &clear);
                if second.is_some() || bits != before {
                    meta.deletions = None;
                    meta.set_deletions(origin, bits, &mut written);
                }
            }
            metas.push(meta);
        }
        let index = Index {
            watermark: self.watermark.min(other.watermark),
            segments: metas,
            retired: retired.into_iter().map(|(id, at)| Retired { id, at }).collect(),
            removed_blocks: removed_blocks.into_values().collect(),
            table: dir,
        };
        let kept: HashSet<_> = index.parts().into_iter().collect();
        written.dropped = self
            .parts()
            .into_iter()
            .chain(other.parts())
            .filter(|name| !kept.contains(name))
            .collect();
        written.tidy();
        Ok((index, written))
    }
}
