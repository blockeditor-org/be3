//! Queries: every clause must match a document's name or body; BM25 ranks
//! them, a name's hits boosted, the newer commit first on equal scores.
//!
//! Each segment is searched document at a time over its clauses' posting
//! cursors, rarest clause first. Before reading a postings block the cursors'
//! skip data bounds the score any document up to the nearest block end can
//! reach; when that cannot enter the results the whole range is skipped
//! unread (block-max WAND over a conjunction).

use std::collections::{BTreeMap, HashMap};

use crate::{
    BODY, DocId, Error, Index, NAME, PartKind, PartName, Parts,
    segment::{self, Bits, Dictionary, Meta, Posting, Skip, TermInfo},
    tokens::{fold, key, terms},
};

/// How much more a hit in the name counts than one in the body.
const NAME_BOOST: f64 = 5.0;
const K1: f64 = 1.2;
const B: f64 = 0.75;
/// A prefix expands to at most this many terms, the most frequent.
const EXPANSIONS: usize = 16;
/// The dictionary blocks per segment and field a prefix reads at most.
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
    /// The time of the commit it was indexed at.
    pub time: i64,
    pub score: f64,
}

fn idf(docs: u64, df: u64) -> f64 {
    let (docs, df) = (docs as f64, df as f64);
    (1.0 + (docs - df + 0.5) / (df + 0.5)).ln()
}

fn tf_part(tf: u32, length: f64, average: f64) -> f64 {
    let tf = f64::from(tf);
    tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * length / average))
}

/// Dictionary lookups for one segment, each block read once.
struct Lookup<'a> {
    dictionary: Dictionary<'a>,
    blocks: HashMap<usize, Vec<(Vec<u8>, TermInfo)>>,
}

impl Lookup<'_> {
    async fn block(
        &mut self,
        parts: &impl Parts,
        index: usize,
    ) -> Result<&[(Vec<u8>, TermInfo)], Error> {
        if !self.blocks.contains_key(&index) {
            let block = self.dictionary.read_block(parts, index).await?;
            self.blocks.insert(index, block);
        }
        Ok(&self.blocks[&index])
    }

    async fn info(&mut self, parts: &impl Parts, key: &[u8]) -> Result<Option<TermInfo>, Error> {
        let Some(index) = self.dictionary.block_of(key) else {
            return Ok(None);
        };
        let block = self.block(parts, index).await?;
        Ok(block
            .binary_search_by(|(term, _)| term.as_slice().cmp(key))
            .ok()
            .map(|at| block[at].1))
    }
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

impl Cursor {
    async fn open(
        parts: &impl Parts,
        meta: &Meta,
        info: TermInfo,
        field: u8,
        weight: f64,
        average: f64,
    ) -> Result<Self, Error> {
        let postings = meta.part(PartKind::Postings);
        let single = info.df as usize <= segment::POSTINGS_BLOCK;
        let len = if single {
            info.postings_len
        } else {
            info.skip_len
        };
        let bytes = parts
            .read(&postings, info.postings..info.postings + u64::from(len))
            .await?;
        let skip_bytes = bytes
            .get(..info.skip_len as usize)
            .ok_or(Error::Corrupt("skip data"))?;
        let skip = segment::decode_skip(skip_bytes)?;
        let mut offsets = Vec::with_capacity(skip.len());
        let (mut at, mut positions_at) = (info.postings + u64::from(info.skip_len), info.positions);
        for entry in &skip {
            offsets.push((at, positions_at));
            at += u64::from(entry.len);
            positions_at += u64::from(entry.positions_len);
        }
        let bounds = skip
            .iter()
            .map(|entry| {
                weight * tf_part(entry.max_tf, segment::decode_norm(entry.min_norm), average)
            })
            .collect();
        let loaded = if single {
            Some((
                0,
                segment::decode_block(&bytes[info.skip_len as usize..], info.df as usize, 0)?,
                false,
            ))
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
                let end = start + u64::from(self.skip[self.block].len);
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
            let end = start + u64::from(self.skip[*index].positions_len);
            let bytes = parts.read(&self.positions, start..end).await?;
            segment::decode_positions(&bytes, list)?;
            *read = true;
        }
        Ok(list[self.at].positions.clone())
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
struct Plan {
    conjuncts: Vec<Vec<String>>,
    phrases: Vec<Vec<usize>>,
}

impl Index {
    pub async fn search(
        &self,
        parts: &impl Parts,
        query: &Query,
        limit: usize,
    ) -> Result<Vec<Hit>, Error> {
        if query.clauses.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let mut lookups = Vec::with_capacity(self.segments.len());
        for meta in &self.segments {
            lookups.push(Lookup {
                dictionary: meta.dictionary()?,
                blocks: HashMap::new(),
            });
        }
        let mut plan = Plan {
            conjuncts: Vec::new(),
            phrases: Vec::new(),
        };
        for clause in &query.clauses {
            match clause {
                Clause::Word(word) => plan.conjuncts.push(vec![fold(word)]),
                Clause::Prefix(prefix) => {
                    let expanded = expand(parts, &mut lookups, &fold(prefix)).await?;
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
        // Each term's information per segment, and the stats over them all.
        let mut infos: Vec<BTreeMap<Vec<u8>, TermInfo>> =
            vec![BTreeMap::new(); self.segments.len()];
        let mut df: BTreeMap<Vec<u8>, u64> = BTreeMap::new();
        for (segment, lookup) in lookups.iter_mut().enumerate() {
            for term in plan.conjuncts.iter().flatten() {
                for field in [NAME, BODY] {
                    let term_key = key(field, term);
                    if let Some(info) = lookup.info(parts, &term_key).await? {
                        *df.entry(term_key.clone()).or_default() += u64::from(info.df);
                        infos[segment].insert(term_key, info);
                    }
                }
            }
        }
        let docs: u64 = self
            .segments
            .iter()
            .map(|meta| u64::from(meta.live()))
            .sum();
        let all: u64 = self
            .segments
            .iter()
            .map(|meta| u64::from(meta.docs))
            .sum::<u64>()
            .max(1);
        let averages = [NAME, BODY].map(|field| {
            let sum: u64 = self
                .segments
                .iter()
                .map(|meta| meta.lengths[field as usize])
                .sum();
            (sum as f64 / all as f64).max(1.0)
        });
        let mut found: Vec<(f64, usize, u32)> = Vec::new();
        for (segment, meta) in self.segments.iter().enumerate() {
            let mut conjuncts = Vec::with_capacity(plan.conjuncts.len());
            for terms in &plan.conjuncts {
                let mut children = Vec::new();
                for term in terms {
                    for field in [NAME, BODY] {
                        let term_key = key(field, term);
                        let Some(info) = infos[segment].get(&term_key) else {
                            continue;
                        };
                        let boost = if field == NAME { NAME_BOOST } else { 1.0 };
                        let weight = boost * idf(docs.max(1), df[&term_key]);
                        let average = averages[field as usize];
                        children
                            .push(Cursor::open(parts, meta, *info, field, weight, average).await?);
                    }
                }
                conjuncts.push(Conjunct { children });
            }
            if conjuncts
                .iter()
                .any(|conjunct| conjunct.children.is_empty())
            {
                continue;
            }
            let deletions = meta.deletions(parts).await?;
            let threshold = kth(&found, limit);
            let best = search_segment(
                parts,
                &mut conjuncts,
                &plan.phrases,
                &deletions,
                limit,
                threshold,
            )
            .await?;
            found.extend(
                best.into_iter()
                    .map(|(score, local)| (score, segment, local)),
            );
        }
        let mut hits = Vec::with_capacity(found.len());
        for (score, segment, local) in found {
            let document = self.segments[segment].document(parts, local).await?;
            hits.push(Hit {
                doc: document.doc,
                time: document.time,
                score,
            });
        }
        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then(b.time.cmp(&a.time))
                .then_with(|| a.doc.cmp(&b.doc))
        });
        hits.truncate(limit);
        Ok(hits)
    }
}

/// The `limit`th best score so far, once there are that many.
fn kth(found: &[(f64, usize, u32)], limit: usize) -> f64 {
    if found.len() < limit {
        return f64::NEG_INFINITY;
    }
    let mut scores: Vec<f64> = found.iter().map(|(score, ..)| *score).collect();
    scores.sort_by(|a, b| b.total_cmp(a));
    scores[limit - 1]
}

/// The terms a prefix expands to: those starting with it in either field,
/// the most frequent first.
async fn expand(
    parts: &impl Parts,
    lookups: &mut [Lookup<'_>],
    prefix: &str,
) -> Result<Vec<String>, Error> {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for lookup in lookups.iter_mut() {
        for field in [NAME, BODY] {
            let prefix_key = key(field, prefix);
            for index in lookup.dictionary.prefix_blocks(&prefix_key, PREFIX_BLOCKS) {
                for (term, info) in lookup.block(parts, index).await? {
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
    Ok(terms.into_iter().map(|(term, _)| term).collect())
}

/// One segment's best `limit` documents, by score then local id (newest
/// first), skipping documents that cannot reach `threshold`. A block's bound
/// is the same sum of the same function as a score, of the block's highest
/// frequency and shortest field, so it is never below a score it bounds.
async fn search_segment(
    parts: &impl Parts,
    conjuncts: &mut [Conjunct],
    phrases: &[Vec<usize>],
    deletions: &Bits,
    limit: usize,
    threshold: f64,
) -> Result<Vec<(f64, u32)>, Error> {
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
        if bound < threshold || (full && bound <= best[limit - 1].0) {
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
                lists
                    .iter()
                    .enumerate()
                    .skip(1)
                    .all(|(offset, list)| list.binary_search(&(start + offset as u32)).is_ok())
            });
            if found {
                continue 'phrases;
            }
        }
        return Ok(false);
    }
    Ok(true)
}
