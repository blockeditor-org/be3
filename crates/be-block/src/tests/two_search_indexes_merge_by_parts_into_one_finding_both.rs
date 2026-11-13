use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

use be_block_commit::MergeResult;
use be_lib_search::{Batch, Commit, DocId, Put, Query, Text};

use crate::search_index::SearchIndexContent;
use crate::{BlockContent, MergeParts, PartReader};

/// Runs a future that never waits: every part here is held in memory.
fn run<T>(future: impl Future<Output = T>) -> T {
    match pin!(future)
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("a read from memory waited"),
    }
}

fn unread() -> PartReader {
    PartReader::new(|_, _, _| Box::pin(async { Err("every part is held".to_owned()) }))
}

fn put(block: u8, time: i64, body: &str) -> Put {
    Put {
        doc: DocId {
            block: vec![block; 16],
            key: String::new(),
        },
        commit: Commit {
            time,
            id: [block; 32],
        },
        source: None,
        text: Some(Text {
            name: "Note".to_owned(),
            body: body.to_owned(),
        }),
        partial: false,
    }
}

fn found(index: &SearchIndexContent, text: &str) -> Vec<u8> {
    let hits = run(index.search(&unread(), &Query::parse(text), 10, i64::MAX)).unwrap();
    hits.iter().map(|hit| hit.doc.block[0]).collect()
}

fn applied(base: &SearchIndexContent, origin: u8, puts: Vec<Put>) -> SearchIndexContent {
    let mut index = base.clone();
    let batch = Batch {
        put: puts,
        ..Batch::default()
    };
    assert_eq!(
        run(index.apply(&unread(), [origin; 16], batch, i64::MAX)),
        Ok(Vec::new())
    );
    index
}

#[test]
fn two_search_indexes_merge_by_parts_into_one_finding_both() {
    let base = applied(&SearchIndexContent::default(), 1, vec![put(1, 1, "apples")]);
    let ours = applied(&base, 2, vec![put(2, 2, "bananas")]);
    let theirs = applied(&base, 3, vec![put(3, 3, "cherries"), put(1, 4, "dates")]);
    let decoded = SearchIndexContent::decode(&ours.encode()).unwrap();
    assert_eq!(found(&decoded, "bananas"), [2]);

    let merged = match run(SearchIndexContent::merge_parts(
        base,
        ours,
        theirs,
        unread(),
    )) {
        Ok(MergeResult::Clean(merged)) => merged,
        other => panic!("the indexes did not merge cleanly: {:?}", other.err()),
    };
    assert_eq!(found(&merged, "bananas"), [2]);
    assert_eq!(found(&merged, "cherries"), [3]);
    assert_eq!(found(&merged, "dates"), [1]);
    assert!(
        found(&merged, "apples").is_empty(),
        "a document kept its words from before it was indexed again"
    );
}
