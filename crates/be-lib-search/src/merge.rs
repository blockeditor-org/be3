//! Merging two indexes, as two devices' offline batches meet: the union of
//! their segments by id (less those either retired), the newest entry per
//! document, the lower watermark. The loser of each document is masked, so
//! it never conflicts.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::{
    Entry, Error, Id, Index, Parts, Place, Written,
    segment::{Bits, Meta},
    table::{self, ChunkRef},
};

impl Index {
    /// The union of `self` and `other`, whose parts `parts` reads alike,
    /// writing what changed under `origin`.
    pub async fn merge(&self, other: &Index, parts: &impl Parts, origin: Id) -> Result<(Index, Written), Error> {
        let mut written = Written::default();
        let retired: BTreeSet<Id> = self.retired.iter().chain(&other.retired).copied().collect();
        let mut segments: Vec<(Meta, Option<Meta>)> = Vec::new();
        for meta in self.segments.iter().chain(&other.segments) {
            if retired.contains(&meta.id) {
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

        // The table: chunks both name are the same; the rest merge by entry.
        let ours: HashSet<_> = self.table.iter().map(|chunk| chunk.part).collect();
        let theirs: HashSet<_> = other.table.iter().map(|chunk| chunk.part).collect();
        let shared: Vec<ChunkRef> = self
            .table
            .iter()
            .filter(|chunk| theirs.contains(&chunk.part))
            .cloned()
            .collect();
        let mut merged: BTreeMap<crate::DocId, Entry> = BTreeMap::new();
        let mut masks: Vec<Place> = Vec::new();
        let unshared = self
            .table
            .iter()
            .filter(|chunk| !theirs.contains(&chunk.part))
            .chain(other.table.iter().filter(|chunk| !ours.contains(&chunk.part)));
        for chunk in unshared {
            for entry in table::read_chunk(parts, chunk).await? {
                let Some(old) = merged.remove(&entry.doc) else {
                    merged.insert(entry.doc.clone(), entry);
                    continue;
                };
                let rank = |entry: &Entry| {
                    let survives = entry.place.is_none_or(|place| surviving.contains(&place.segment));
                    (entry.commit, survives, std::cmp::Reverse(entry.place))
                };
                let (winner, loser) = if rank(&entry) > rank(&old) { (entry, old) } else { (old, entry) };
                if let Some(place) = loser.place
                    && loser.place != winner.place
                    && surviving.contains(&place.segment)
                {
                    masks.push(place);
                }
                merged.insert(winner.doc.clone(), winner);
            }
        }
        let mut dir = Vec::new();
        let mut next = 0;
        let mut pending: Vec<Entry> = Vec::new();
        let mut entries = merged.into_values().peekable();
        for chunk in shared {
            while let Some(entry) = entries.next_if(|entry| entry.doc < chunk.first) {
                pending.push(entry);
            }
            dir.extend(table::write_chunks(&pending, origin, &mut next, &mut written));
            pending.clear();
            dir.push(chunk);
        }
        pending.extend(entries);
        dir.extend(table::write_chunks(&pending, origin, &mut next, &mut written));

        // Deletion bitmaps: both sides' bits and the losers'.
        let mut metas = Vec::with_capacity(segments.len());
        for (mut meta, second) in segments {
            let locals: Vec<u32> = masks
                .iter()
                .filter(|place| place.segment == meta.id)
                .map(|place| place.local)
                .collect();
            if second.is_some() || !locals.is_empty() {
                let mut bits: Bits = meta.deletions(parts).await?;
                if let Some(second) = &second {
                    bits.union(&second.deletions(parts).await?);
                }
                for local in locals {
                    bits.set(local);
                }
                meta.deletions = None;
                meta.set_deletions(origin, bits, &mut written);
            }
            metas.push(meta);
        }
        let index = Index {
            watermark: self.watermark.min(other.watermark),
            segments: metas,
            retired: retired.into_iter().collect(),
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
