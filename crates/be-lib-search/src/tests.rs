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

mod a_query_reads_few_blocks_while_skipping_a_common_term;
mod a_reindexed_document_no_longer_matches_its_old_words;
mod compaction_gives_the_same_answers_as_one_index_built_from_scratch;
mod finds_each_document_by_a_word_a_prefix_and_a_phrase;
mod headers_and_part_names_round_trip;
mod merging_two_indexes_gives_the_same_answers_as_one_built_from_scratch;
mod names_rank_first_then_newer_commits;
mod queries_parse_words_phrases_and_a_last_prefix;

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
    async fn read(&self, part: &PartName, range: Range<u64>) -> Result<Vec<u8>, Error> {
        self.reads.borrow_mut().push(part.kind);
        self.parts.read(part, range).await
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
