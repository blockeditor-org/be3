use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    future::Future,
    ops::Range,
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::{
    Batch, Commit, DocId, Error, Id, Index, MemoryParts, PartKind, PartName, Parts, Put, Query,
    Text, Written,
};

mod a_put_without_text_survives_a_merge_with_compaction_or_reindexing;
mod a_query_reads_few_blocks_while_skipping_a_common_term;
mod a_reindexed_document_no_longer_matches_its_old_words;
mod a_snippet_cuts_at_wide_whitespace_without_splitting_a_character;
mod a_snippet_marks_each_match_around_the_first;
mod compaction_gives_the_same_answers_as_one_index_built_from_scratch;
mod corrupt_parts_are_errors_not_panics;
mod finds_each_document_by_a_word_a_prefix_and_a_phrase;
mod headers_and_part_names_round_trip;
mod merging_two_indexes_gives_the_same_answers_as_one_built_from_scratch;
mod names_rank_first_then_newer_commits;
mod queries_parse_words_phrases_and_a_last_prefix;
mod ranked_results_match_a_brute_force_scorer;
mod removals_of_unseen_documents_survive_a_merge;

/// Runs a future that never waits: every read here is from memory.
pub(crate) fn run<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("a read from memory waited"),
    }
}

/// Every part any index here wrote, as a block's object store holds them,
/// counting reads by kind.
#[derive(Default)]
pub(crate) struct Store {
    pub(crate) parts: MemoryParts,
    pub(crate) reads: RefCell<Vec<PartKind>>,
}

impl Store {
    pub(crate) fn keep(&mut self, written: &Written) {
        self.parts.insert(written);
    }

    pub(crate) fn count(&self, kind: fn(&PartKind) -> bool) -> usize {
        self.reads.borrow().iter().filter(|read| kind(read)).count()
    }
}

impl Parts for Store {
    fn read<'a>(
        &'a self,
        part: &'a PartName,
        range: Range<u64>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, Error>> + 'a>> {
        self.reads.borrow_mut().push(part.kind);
        self.parts.read(part, range)
    }
}

pub(crate) fn id(n: u32) -> Id {
    let mut id = [0x5a; 16];
    id[..4].copy_from_slice(&n.to_le_bytes());
    id
}

pub(crate) fn doc(n: u32) -> DocId {
    DocId {
        block: n.to_be_bytes().to_vec(),
        key: String::new(),
    }
}

pub(crate) fn commit(time: i64) -> Commit {
    let mut id = [0; 32];
    id[..8].copy_from_slice(&time.to_le_bytes());
    Commit { time, id }
}

const VOCABULARY: &[&str] = &[
    "river", "stone", "cloud", "lantern", "harbor", "meadow", "copper", "willow", "ember",
    "falcon", "glacier", "orchard", "pepper", "quartz", "saddle", "thistle", "violet", "walnut",
    "yonder", "zephyr",
];

/// Document `n`'s text, version `version`: common words, a unique word
/// `uniq{n}v{version}x`, and the phrase `mark{n} sign{n}`.
pub(crate) fn text(n: u32, version: u32) -> Text {
    let mut seed = n.wrapping_mul(2654435761).wrapping_add(version);
    let mut words: Vec<String> = (0..24)
        .map(|_| {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            VOCABULARY[(seed >> 16) as usize % VOCABULARY.len()].to_owned()
        })
        .collect();
    let at = (seed >> 8) as usize % words.len();
    words.insert(at, format!("Mark{n} sign{n}"));
    words.push(format!("uniq{n}v{version}x"));
    Text {
        name: format!("Note {n}"),
        body: words.join(" "),
    }
}

pub(crate) fn put(n: u32, version: u32, time: i64) -> Put {
    Put {
        doc: doc(n),
        commit: commit(time),
        source: None,
        text: Some(text(n, version)),
    }
}

pub(crate) fn apply(index: &mut Index, store: &mut Store, origin: Id, batch: Batch) {
    let written = run(index.apply(store, origin, batch)).unwrap();
    store.keep(&written);
}

/// An index of `documents` built in one batch.
pub(crate) fn from_scratch(documents: &BTreeMap<u32, (u32, i64)>, store: &mut Store) -> Index {
    let mut index = Index::default();
    let batch = Batch {
        put: documents
            .iter()
            .map(|(&n, &(version, time))| put(n, version, time))
            .collect(),
        ..Batch::default()
    };
    apply(&mut index, store, id(999_999), batch);
    index
}

pub(crate) fn found(index: &Index, store: &Store, query: &str) -> BTreeSet<DocId> {
    run(index.search(store, &Query::parse(query), 10_000))
        .unwrap()
        .into_iter()
        .map(|hit| hit.doc)
        .collect()
}

/// Queries over the documents numbered below `count`: words, prefixes,
/// phrases, common words.
pub(crate) fn queries(count: u32) -> Vec<String> {
    let mut out: Vec<String> = VOCABULARY.iter().map(|word| word.to_string()).collect();
    out.push("river stone".into());
    out.push("note 1".into());
    out.push("\"river stone\"".into());
    for n in (0..count).step_by(7) {
        // Prefixes here expand to few terms: a prefix's expansion counts
        // masked documents' terms too, so a wide one differs by layout.
        out.push(format!("mark{n} "));
        out.push(format!("\"mark{n} sign{n}\""));
        out.push(format!("uniq{n}v"));
        for version in 0..3 {
            out.push(format!("uniq{n}v{version}x "));
        }
    }
    out
}

pub(crate) fn same_answers(a: (&Index, &Store), b: (&Index, &Store), count: u32) {
    for query in queries(count) {
        assert_eq!(
            found(a.0, a.1, &query),
            found(b.0, b.1, &query),
            "{query:?}"
        );
    }
}

/// Hits in order with their scores.
pub(crate) fn ranked(index: &Index, store: &Store, query: &str, limit: usize) -> Vec<(DocId, f64)> {
    run(index.search(store, &Query::parse(query), limit))
        .unwrap()
        .into_iter()
        .map(|hit| (hit.doc, hit.score))
        .collect()
}

/// What `search` should answer for a query of words and phrases, scoring
/// every live document of every segment in full.
pub(crate) fn brute_force(
    index: &Index,
    store: &Store,
    query: &str,
    limit: usize,
) -> Vec<(DocId, f64)> {
    use crate::{
        BODY, NAME,
        query::{Clause, Stats, boost, idf, tf_part},
        segment::decode_norm,
        tokens::key,
    };
    let mut conjuncts: Vec<String> = Vec::new();
    let mut phrases: Vec<Vec<usize>> = Vec::new();
    for clause in Query::parse(query).clauses {
        match clause {
            Clause::Word(word) => conjuncts.push(word),
            Clause::Phrase(words) => {
                let start = conjuncts.len();
                conjuncts.extend(words);
                phrases.push((start..conjuncts.len()).collect());
            }
            Clause::Prefix(_) => panic!("the brute force scorer takes no prefixes"),
        }
    }
    let stats = Stats::of(&index.segments);
    let segments: Vec<_> = index
        .segments
        .iter()
        .map(|meta| {
            let documents = run(meta.documents(store)).unwrap();
            let deletions = run(meta.deletions(store)).unwrap();
            let terms = run(meta.read_all(store)).unwrap();
            (documents, deletions, terms)
        })
        .collect();
    let mut df: BTreeMap<Vec<u8>, u64> = BTreeMap::new();
    for (_, _, terms) in &segments {
        for (term, list) in terms {
            *df.entry(term.clone()).or_default() += list.len() as u64;
        }
    }
    let mut hits: Vec<(f64, Commit, DocId)> = Vec::new();
    for (documents, deletions, terms) in &segments {
        for (local, document) in documents.iter().enumerate() {
            let local = local as u32;
            if deletions.get(local) {
                continue;
            }
            let posting = |term: &str, field: u8| {
                terms
                    .get(&key(field, term))
                    .and_then(|list| list.iter().find(|posting| posting.doc == local))
            };
            if conjuncts
                .iter()
                .any(|term| posting(term, NAME).is_none() && posting(term, BODY).is_none())
            {
                continue;
            }
            let phrase_found = phrases.iter().all(|phrase| {
                [NAME, BODY].into_iter().any(|field| {
                    let lists: Option<Vec<&Vec<u32>>> = phrase
                        .iter()
                        .map(|&index| {
                            posting(&conjuncts[index], field).map(|posting| &posting.positions)
                        })
                        .collect();
                    lists.is_some_and(|lists| {
                        lists[0].iter().any(|&start| {
                            lists
                                .iter()
                                .enumerate()
                                .all(|(offset, list)| list.contains(&(start + offset as u32)))
                        })
                    })
                })
            });
            if !phrase_found {
                continue;
            }
            let score: f64 = conjuncts
                .iter()
                .map(|term| {
                    [NAME, BODY]
                        .into_iter()
                        .filter_map(|field| {
                            let term_key = key(field, term);
                            terms.get(&term_key)?;
                            let weight = boost(field) * idf(stats.docs, df[&term_key]);
                            Some(posting(term, field).map_or(0.0, |posting| {
                                weight
                                    * tf_part(
                                        posting.tf,
                                        decode_norm(posting.norm),
                                        stats.averages[field as usize],
                                    )
                            }))
                        })
                        .sum::<f64>()
                })
                .sum();
            hits.push((score, document.commit, document.doc.clone()));
        }
    }
    hits.sort_by(|a, b| {
        b.0.total_cmp(&a.0)
            .then(b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    hits.truncate(limit);
    hits.into_iter()
        .map(|(score, _, doc)| (doc, score))
        .collect()
}

/// That `search` and the brute force scorer agree, order and scores.
pub(crate) fn agrees_with_brute_force(index: &Index, store: &Store, query: &str, limit: usize) {
    let found = ranked(index, store, query, limit);
    let expected = brute_force(index, store, query, limit);
    assert_eq!(
        found.iter().map(|(doc, _)| doc).collect::<Vec<_>>(),
        expected.iter().map(|(doc, _)| doc).collect::<Vec<_>>(),
        "{query:?} limit {limit}: {found:?} against {expected:?}"
    );
    for ((_, a), (_, b)) in found.iter().zip(&expected) {
        assert!((a - b).abs() < 1e-9, "{query:?}: score {a} against {b}");
    }
}
