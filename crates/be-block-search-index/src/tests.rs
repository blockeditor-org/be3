use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

pub(crate) use be_block_commit::MergeResult;
pub(crate) use be_lib_search::{Batch, Commit, DocId, Put, Query, Text};

pub(crate) use crate::SearchIndexContent;
pub(crate) use be_block_content::{BlockContent, MergeParts, PartReader};

/// Runs a future that never waits: every part here is held in memory.
pub(crate) fn run<T>(future: impl Future<Output = T>) -> T {
    match pin!(future)
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("a read from memory waited"),
    }
}

pub(crate) fn unread() -> PartReader {
    PartReader::new(|_, _, _| Box::pin(async { Err("every part is held".to_owned()) }))
}

pub(crate) fn put(block: u8, time: i64, body: &str) -> Put {
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

pub(crate) fn found(index: &SearchIndexContent, text: &str) -> Vec<u8> {
    let hits = run(index.search(&unread(), &Query::parse(text), 10, i64::MAX)).unwrap();
    hits.iter().map(|hit| hit.doc.block[0]).collect()
}

pub(crate) fn applied(base: &SearchIndexContent, origin: u8, puts: Vec<Put>) -> SearchIndexContent {
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

mod a_compaction_saves_its_merged_parts_through_a_part_writer;
mod two_search_indexes_merge_by_parts_into_one_finding_both;
