//! Queries: every clause must match a document's name or body; BM25 ranks
//! them, a name's hits boosted, the newer commit first on equal scores.
//!
//! Each segment is searched document at a time over its clauses' posting
//! cursors, rarest clause first. Before reading a postings block the cursors'
//! skip data bounds the score any document up to the nearest block end can
//! reach; when that cannot enter the results the whole range is skipped
//! unread (block-max WAND over a conjunction).
//!
//! A query's reads are a few rounds, each joined over every segment: the
//! dictionary blocks prefixes need, those of every term, the terms' skip data
//! and deletion bitmaps, then each segment's postings blocks as its search
//! needs them (the segments searched together, sharing the score to beat),
//! then the documents of the hits shown.
//!
//! Statistics count every document a segment holds, masked ones too, until
//! compaction drops them: document frequencies come from postings, which
//! cannot tell, so the document count must not either, or a term's idf could
//! go below zero.

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
};

use crate::{
    BODY, Commit, DocId, Error, Index, NAME, PartKind, PartName, Parts, join,
    segment::{self, Bits, Dictionary, Meta, Posting, Skip, TermInfo},
    tokens::{fold, key, terms},
};

/// How much more a hit in the name counts than one in the body.
const NAME_BOOST: f64 = 5.0;
const K1: f64 = 1.2;
const B: f64 = 0.75;
/// A prefix expands to at most this many terms, the most frequent.
const EXPANSIONS: usize = 16;
/// The dictionary blocks per segment and field a prefix reads at most: those
/// whose most frequent term is the most frequent.
const PREFIX_BLOCKS: usize = 4;
/// Prefixes start at this many characters; a shorter last word is a word.
const SHORTEST_PREFIX: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Clause {
    Word(String),
    /// Any term starting with this.
    Prefix(String),
    /// These words in a row in one field.
    Phrase(Vec<String>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub clauses: Vec<Clause>,
}

impl Query {
    /// Words, with `"quoted words"` as phrases; the last word is a prefix
    /// unless the text ends after it.
    pub fn parse(text: &str) -> Self {
        let pieces: Vec<&str> = text.split('"').collect();
        let mut clauses = Vec::new();
        let mut last_outside = false;
        for (index, piece) in pieces.iter().enumerate() {
            let quoted = index % 2 == 1 && index + 1 < pieces.len();
            let words: Vec<String> = terms(piece).collect();
            if words.is_empty() {
                continue;
            }
            if quoted && words.len() > 1 {
                clauses.push(Clause::Phrase(words));
            } else {
                clauses.extend(words.into_iter().map(Clause::Word));
            }
            last_outside = !quoted;
        }
        let open_ended = text
            .chars()
            .last()
            .is_some_and(|c| !c.is_whitespace() && c != '"');
        if open_ended
            && last_outside
            && let Some(Clause::Word(word)) = clauses.last()
            && word.chars().count() >= SHORTEST_PREFIX
        {
            let word = word.clone();
            *clauses.last_mut().unwrap() = Clause::Prefix(word);
        }
        Self { clauses }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub doc: DocId,
    /// The commit its text was indexed at, by which hits of two indexes are
    /// told apart.
    pub commit: Commit,
    pub score: f64,
}

pub(crate) fn idf(docs: u64, df: u64) -> f64 {
    let (docs, df) = (docs as f64, df.min(docs) as f64);
    (1.0 + (docs - df + 0.5) / (df + 0.5)).ln()
}

pub(crate) fn tf_part(tf: u32, length: f64, average: f64) -> f64 {
    let tf = f64::from(tf);
    tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * length / average))
}

pub(crate) fn boost(field: u8) -> f64 {
    if field == NAME { NAME_BOOST } else { 1.0 }
}

/// The global statistics a query scores with.
pub(crate) struct Stats {
    pub(crate) docs: u64,
    pub(crate) averages: [f64; 2],
}

impl Stats {
    pub(crate) fn of(segments: &[Meta]) -> Self {
        let docs: u64 = segments
            .iter()
            .map(|meta| u64::from(meta.docs))
            .sum::<u64>()
            .max(1);
        let averages = [NAME, BODY].map(|field| {
            let sum: u64 = segments
                .iter()
                .map(|meta| meta.lengths[field as usize])
                .fold(0, u64::saturating_add);
            (sum as f64 / docs as f64).max(1.0)
        });
        Self { docs, averages }
    }
}

/// Dictionary blocks read for one segment.
struct Lookup<'a> {
    dictionary: Dictionary<'a>,
    blocks: HashMap<usize, Vec<(Vec<u8>, TermInfo)>>,
}

impl Lookup<'_> {
    /// A term's information, if its block was read and holds it.
    fn info(&self, key: &[u8]) -> Option<TermInfo> {
        let block = self.blocks.get(&self.dictionary.block_of(key)?)?;
        block
            .binary_search_by(|(term, _)| term.as_slice().cmp(key))
            .ok()
            .map(|at| block[at].1)
    }
}

/// Reads the dictionary blocks in `wanted` (segment, block) that are not read
/// yet, all at once.
async fn load_blocks(
    parts: &impl Parts,
    lookups: &mut [Lookup<'_>],
    mut wanted: Vec<(usize, usize)>,
) -> Result<(), Error> {
    wanted.retain(|(segment, block)| !lookups[*segment].blocks.contains_key(block));
    wanted.sort_unstable();
    wanted.dedup();
    let dictionaries: Vec<Dictionary<'_>> =
        lookups.iter().map(|lookup| lookup.dictionary).collect();
    let read = join::try_all(
        wanted
            .iter()
            .map(|&(segment, block)| dictionaries[segment].read_block(parts, block)),
    )
    .await?;
    for ((segment, block), entries) in wanted.into_iter().zip(read) {
        lookups[segment].blocks.insert(block, entries);
    }
    Ok(())
}

/// One term's postings in one segment, read a block at a time.
struct Cursor {
    postings: PartName,
    positions: PartName,
    info: TermInfo,
    skip: Vec<Skip>,
    /// Each block's offset in the postings part and in the positions part.
    offsets: Vec<(u64, u64)>,
    bounds: Vec<f64>,
    weight: f64,
    field: u8,
    average: f64,
    block: usize,
    loaded: Option<(usize, Vec<Posting>, bool)>,
    at: usize,
}

/// What a cursor is opened on.
#[derive(Clone, Copy)]
struct Spec {
    info: TermInfo,
    field: u8,
    weight: f64,
    average: f64,
}

impl Cursor {
    async fn open(parts: &impl Parts, meta: &Meta, spec: Spec) -> Result<Self, Error> {
        let Spec {
            info,
            field,
            weight,
            average,
        } = spec;
        let postings = meta.part(PartKind::Postings);
        let single = info.df as usize <= segment::POSTINGS_BLOCK;
        let len = if single {
            info.postings_len
        } else {
            info.skip_len
        };
        if info.skip_len > info.postings_len {
            return Err(Error::Corrupt("term information"));
        }
        let end = segment::add(info.postings, u64::from(len))?;
        let bytes = parts.read(&postings, info.postings..end).await?;
        let skip_bytes = bytes
            .get(..info.skip_len as usize)
            .ok_or(Error::Corrupt("skip data"))?;
        let skip = segment::decode_skip(skip_bytes, info.df)?;
        let mut offsets = Vec::with_capacity(skip.len());
        let (mut at, mut positions_at) = (
            segment::add(info.postings, u64::from(info.skip_len))?,
            info.positions,
        );
        for entry in &skip {
            offsets.push((at, positions_at));
            at = segment::add(at, u64::from(entry.len))?;
            positions_at = segment::add(positions_at, u64::from(entry.positions_len))?;
        }
        let bounds = skip
            .iter()
            .map(|entry| {
                weight * tf_part(entry.max_tf, segment::decode_norm(entry.min_norm), average)
            })
            .collect();
        let loaded = if single && !skip.is_empty() {
            let block = bytes
                .get(info.skip_len as usize..)
                .ok_or(Error::Corrupt("postings"))?;
            Some((0, segment::decode_block(block, info.df as usize, 0)?, false))
        } else {
            None
        };
        Ok(Self {
            postings,
            positions: meta.part(PartKind::Positions),
            info,
            skip,
            offsets,
            bounds,
            weight,
            field,
            average,
            block: 0,
            loaded,
            at: 0,
        })
    }

    /// Moves to the block that would hold `target`, reading nothing: its
    /// last document and score bound, or None past the end.
    fn shallow(&mut self, target: u32) -> Option<(u32, f64)> {
        while self.block < self.skip.len() && self.skip[self.block].last < target {
            self.block += 1;
        }
        (self.block < self.skip.len())
            .then(|| (self.skip[self.block].last, self.bounds[self.block]))
    }

    async fn next_geq(&mut self, parts: &impl Parts, target: u32) -> Result<Option<u32>, Error> {
        loop {
            if self.shallow(target).is_none() {
                return Ok(None);
            }
            if self
                .loaded
                .as_ref()
                .is_none_or(|(index, ..)| *index != self.block)
            {
                let (start, _) = self.offsets[self.block];
                let end = segment::add(start, u64::from(self.skip[self.block].len))?;
                let bytes = parts.read(&self.postings, start..end).await?;
                let previous = self
                    .block
                    .checked_sub(1)
                    .map_or(0, |index| self.skip[index].last);
                let count = segment::block_count(self.info.df, self.block);
                self.loaded = Some((
                    self.block,
                    segment::decode_block(&bytes, count, previous)?,
                    false,
                ));
                self.at = 0;
            }
            let (_, list, _) = self.loaded.as_ref().unwrap();
            while self.at < list.len() && list[self.at].doc < target {
                self.at += 1;
            }
            if let Some(posting) = list.get(self.at) {
                return Ok(Some(posting.doc));
            }
            self.block += 1;
        }
    }

    /// The posting at the cursor if it is `doc`'s.
    fn at(&self, doc: u32) -> Option<&Posting> {
        let (index, list, _) = self.loaded.as_ref()?;
        (*index == self.block)
            .then(|| list.get(self.at))
            .flatten()
            .filter(|posting| posting.doc == doc)
    }

    fn score(&self, doc: u32) -> f64 {
        self.at(doc).map_or(0.0, |posting| {
            self.weight * tf_part(posting.tf, segment::decode_norm(posting.norm), self.average)
        })
    }

    async fn positions(&mut self, parts: &impl Parts) -> Result<Vec<u32>, Error> {
        let Some((index, list, read)) = self.loaded.as_mut() else {
            return Ok(Vec::new());
        };
        if !*read {
            let (_, start) = self.offsets[*index];
            let end = segment::add(start, u64::from(self.skip[*index].positions_len))?;
            let bytes = parts.read(&self.positions, start..end).await?;
            segment::decode_positions(&bytes, list)?;
            *read = true;
        }
        Ok(list
            .get(self.at)
            .map(|posting| posting.positions.clone())
            .unwrap_or_default())
    }
}

/// A clause as a union of its terms' cursors in both fields.
struct Conjunct {
    children: Vec<Cursor>,
}

impl Conjunct {
    fn shallow(&mut self, target: u32) -> Option<(u32, f64)> {
        let mut out: Option<(u32, f64)> = None;
        for child in &mut self.children {
            if let Some((last, bound)) = child.shallow(target) {
                out = Some(match out {
                    Some((other, sum)) => (other.min(last), sum + bound),
                    None => (last, bound),
                });
            }
        }
        out
    }

    async fn next_geq(&mut self, parts: &impl Parts, target: u32) -> Result<Option<u32>, Error> {
        let mut out: Option<u32> = None;
        for child in &mut self.children {
            if let Some(doc) = child.next_geq(parts, target).await? {
                out = Some(out.map_or(doc, |other| other.min(doc)));
            }
        }
        Ok(out)
    }

    fn score(&self, doc: u32) -> f64 {
        self.children.iter().map(|child| child.score(doc)).sum()
    }

    fn df(&self) -> u64 {
        self.children
            .iter()
            .map(|child| u64::from(child.info.df))
            .sum()
    }
}

/// A query's clauses as terms: each conjunct's terms, and phrases as runs of
/// conjuncts.
pub(crate) struct Plan {
    pub(crate) conjuncts: Vec<Vec<String>>,
    pub(crate) phrases: Vec<Vec<usize>>,
}

/// The best scores each segment holds so far, and the `limit`th best of
/// them all, which a document must reach to be shown.
struct Shared {
    limit: usize,
    bests: RefCell<Vec<Vec<f64>>>,
    threshold: Cell<f64>,
}

impl Shared {
    fn update(&self, segment: usize, best: &[(f64, u32)]) {
        let mut bests = self.bests.borrow_mut();
        bests[segment] = best.iter().map(|(score, _)| *score).collect();
        let mut all: Vec<f64> = bests.iter().flatten().copied().collect();
        if all.len() >= self.limit {
            all.sort_by(|a, b| b.total_cmp(a));
            self.threshold.set(all[self.limit - 1]);
        }
    }
}

impl Index {
    pub async fn search(
        &self,
        parts: &impl Parts,
        query: &Query,
        limit: usize,
    ) -> Result<Vec<Hit>, Error> {
        if query.clauses.is_empty() || limit == 0 || self.segments.is_empty() {
            return Ok(Vec::new());
        }
        let mut lookups: Vec<Lookup<'_>> = self
            .segments
            .iter()
            .map(|meta| Lookup {
                dictionary: meta.dictionary(),
                blocks: HashMap::new(),
            })
            .collect();
        // Round one: the blocks prefixes expand from.
        let prefixes: Vec<String> = query
            .clauses
            .iter()
            .filter_map(|clause| match clause {
                Clause::Prefix(prefix) => Some(fold(prefix)),
                _ => None,
            })
            .collect();
        let mut wanted = Vec::new();
        for (segment, lookup) in lookups.iter().enumerate() {
            for prefix in &prefixes {
                for field in [NAME, BODY] {
                    let blocks = lookup
                        .dictionary
                        .prefix_blocks(&key(field, prefix), PREFIX_BLOCKS);
                    wanted.extend(blocks.into_iter().map(|block| (segment, block)));
                }
            }
        }
        load_blocks(parts, &mut lookups, wanted).await?;
        let mut plan = Plan {
            conjuncts: Vec::new(),
            phrases: Vec::new(),
        };
        for clause in &query.clauses {
            match clause {
                Clause::Word(word) => plan.conjuncts.push(vec![fold(word)]),
                Clause::Prefix(prefix) => {
                    let expanded = expand(&lookups, &fold(prefix));
                    if expanded.is_empty() {
                        return Ok(Vec::new());
                    }
                    plan.conjuncts.push(expanded);
                }
                Clause::Phrase(words) => {
                    let start = plan.conjuncts.len();
                    plan.conjuncts
                        .extend(words.iter().map(|word| vec![fold(word)]));
                    plan.phrases.push((start..plan.conjuncts.len()).collect());
                }
            }
        }
        // Round two: every term's block.
        let mut wanted = Vec::new();
        for (segment, lookup) in lookups.iter().enumerate() {
            for term in plan.conjuncts.iter().flatten() {
                for field in [NAME, BODY] {
                    wanted.extend(
                        lookup
                            .dictionary
                            .block_of(&key(field, term))
                            .map(|block| (segment, block)),
                    );
                }
            }
        }
        load_blocks(parts, &mut lookups, wanted).await?;
        let mut df: BTreeMap<Vec<u8>, u64> = BTreeMap::new();
        let mut infos: Vec<BTreeMap<Vec<u8>, TermInfo>> =
            vec![BTreeMap::new(); self.segments.len()];
        for (segment, lookup) in lookups.iter().enumerate() {
            for term in plan.conjuncts.iter().flatten() {
                for field in [NAME, BODY] {
                    let term_key = key(field, term);
                    if let Some(info) = lookup.info(&term_key) {
                        *df.entry(term_key.clone()).or_default() += u64::from(info.df);
                        infos[segment].insert(term_key, info);
                    }
                }
            }
        }
        let stats = Stats::of(&self.segments);
        // Each segment's cursors, where every clause has one.
        let mut specs: Vec<(usize, Vec<Vec<Spec>>)> = Vec::new();
        for (segment, infos) in infos.iter().enumerate() {
            let conjuncts: Vec<Vec<Spec>> = plan
                .conjuncts
                .iter()
                .map(|terms| {
                    terms
                        .iter()
                        .flat_map(|term| [NAME, BODY].map(|field| (key(field, term), field)))
                        .filter_map(|(term_key, field)| {
                            let info = *infos.get(&term_key)?;
                            Some(Spec {
                                info,
                                field,
                                weight: boost(field) * idf(stats.docs, df[&term_key]),
                                average: stats.averages[field as usize],
                            })
                        })
                        .collect()
                })
                .collect();
            if conjuncts.iter().all(|specs| !specs.is_empty()) {
                specs.push((segment, conjuncts));
            }
        }
        // Round three: skip data and deletion bitmaps.
        let opened = join::try_all(specs.iter().flat_map(|(segment, conjuncts)| {
            let meta = &self.segments[*segment];
            conjuncts
                .iter()
                .flatten()
                .map(move |spec| Cursor::open(parts, meta, *spec))
        }))
        .await?;
        let deletions = join::try_all(
            specs
                .iter()
                .map(|(segment, _)| self.segments[*segment].deletions(parts)),
        )
        .await?;
        let mut opened = opened.into_iter();
        let mut searches: Vec<(usize, Vec<Conjunct>, Bits)> = Vec::new();
        for ((segment, conjuncts), deletions) in specs.iter().zip(deletions) {
            let conjuncts = conjuncts
                .iter()
                .map(|specs| Conjunct {
                    children: opened.by_ref().take(specs.len()).collect(),
                })
                .collect();
            searches.push((*segment, conjuncts, deletions));
        }
        // Round four on: the segments' searches together.
        let shared = Shared {
            limit,
            bests: RefCell::new(vec![Vec::new(); searches.len()]),
            threshold: Cell::new(f64::NEG_INFINITY),
        };
        let bests = join::try_all(searches.iter_mut().enumerate().map(
            |(slot, (_, conjuncts, deletions))| {
                search_segment(parts, conjuncts, &plan.phrases, deletions, &shared, slot)
            },
        ))
        .await?;
        let mut found: Vec<(f64, usize, u32)> = Vec::new();
        for ((segment, ..), best) in searches.iter().zip(bests) {
            found.extend(
                best.into_iter()
                    .map(|(score, local)| (score, *segment, local)),
            );
        }
        found.sort_by(|a, b| b.0.total_cmp(&a.0));
        if let Some(&(last, ..)) = found.get(limit - 1) {
            // Past the `limit`th, only ties can still come first by commit.
            found.retain(|(score, ..)| *score >= last);
        }
        let documents = join::try_all(
            found
                .iter()
                .map(|&(_, segment, local)| self.segments[segment].document(parts, local)),
        )
        .await?;
        let mut hits: Vec<Hit> = found
            .into_iter()
            .zip(documents)
            .map(|((score, ..), document)| Hit {
                doc: document.doc,
                commit: document.commit,
                score,
            })
            .collect();
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then(b.commit.cmp(&a.commit))
                .then_with(|| a.doc.cmp(&b.doc))
        });
        hits.truncate(limit);
        Ok(hits)
    }
}

/// The terms a prefix expands to, from the blocks round one read: those
/// starting with it in either field, the most frequent first (masked
/// documents count until compaction drops them).
fn expand(lookups: &[Lookup<'_>], prefix: &str) -> Vec<String> {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for lookup in lookups {
        for field in [NAME, BODY] {
            let prefix_key = key(field, prefix);
            for index in lookup.dictionary.prefix_blocks(&prefix_key, PREFIX_BLOCKS) {
                for (term, info) in lookup.blocks.get(&index).into_iter().flatten() {
                    if term.starts_with(&prefix_key)
                        && let Ok(term) = std::str::from_utf8(&term[1..])
                    {
                        *counts.entry(term.to_owned()).or_default() += u64::from(info.df);
                    }
                }
            }
        }
    }
    let mut terms: Vec<(String, u64)> = counts.into_iter().collect();
    terms.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    terms.truncate(EXPANSIONS);
    terms.into_iter().map(|(term, _)| term).collect()
}

/// One segment's best `shared.limit` documents, by score then local id
/// (newest first), skipping documents that cannot reach the shared
/// threshold. A block's bound is the same sum of the same function as a
/// score, of the block's highest frequency and shortest field, so it is never
/// below a score it bounds.
async fn search_segment(
    parts: &impl Parts,
    conjuncts: &mut [Conjunct],
    phrases: &[Vec<usize>],
    deletions: &Bits,
    shared: &Shared,
    slot: usize,
) -> Result<Vec<(f64, u32)>, Error> {
    let limit = shared.limit;
    let mut order: Vec<usize> = (0..conjuncts.len()).collect();
    order.sort_by_key(|&index| conjuncts[index].df());
    let mut best: Vec<(f64, u32)> = Vec::new();
    let mut doc = 0u32;
    'documents: loop {
        let mut bound = 0.0;
        let mut block_end = u32::MAX;
        for conjunct in conjuncts.iter_mut() {
            let Some((last, conjunct_bound)) = conjunct.shallow(doc) else {
                break 'documents;
            };
            bound += conjunct_bound;
            block_end = block_end.min(last);
        }
        let full = best.len() >= limit;
        if bound < shared.threshold.get() || (full && bound <= best[limit - 1].0) {
            match block_end.checked_add(1) {
                Some(next) => {
                    doc = next;
                    continue;
                }
                None => break,
            }
        }
        for &index in &order {
            match conjuncts[index].next_geq(parts, doc).await? {
                None => break 'documents,
                Some(next) if next > doc => {
                    doc = next;
                    continue 'documents;
                }
                Some(_) => {}
            }
        }
        if !deletions.get(doc) && phrases_match(parts, conjuncts, phrases, doc).await? {
            let score: f64 = conjuncts.iter().map(|conjunct| conjunct.score(doc)).sum();
            if !full || score > best[limit - 1].0 {
                let at = best.partition_point(|(other, _)| *other >= score);
                best.insert(at, (score, doc));
                best.truncate(limit);
                shared.update(slot, &best);
            }
        }
        match doc.checked_add(1) {
            Some(next) => doc = next,
            None => break,
        }
    }
    Ok(best)
}

async fn phrases_match(
    parts: &impl Parts,
    conjuncts: &mut [Conjunct],
    phrases: &[Vec<usize>],
    doc: u32,
) -> Result<bool, Error> {
    'phrases: for phrase in phrases {
        'fields: for field in [NAME, BODY] {
            let mut lists = Vec::with_capacity(phrase.len());
            for &index in phrase {
                let Some(child) = conjuncts[index]
                    .children
                    .iter_mut()
                    .find(|child| child.field == field && child.at(doc).is_some())
                else {
                    continue 'fields;
                };
                lists.push(child.positions(parts).await?);
            }
            let found = lists[0].iter().any(|&start| {
                lists.iter().enumerate().skip(1).all(|(offset, list)| {
                    start
                        .checked_add(offset as u32)
                        .is_some_and(|position| list.binary_search(&position).is_ok())
                })
            });
            if found {
                continue 'phrases;
            }
        }
        return Ok(false);
    }
    Ok(true)
}
