use super::*;

/// A commit time past what the reader believes (`Commit::believed`) counts as the oldest: a
/// forged far-future entry ranks after honest ones on equal scores, and the next honest put
/// replaces it, though by its own time it is the newer.
#[test]
fn a_commit_time_past_the_readers_bound_counts_as_the_oldest() {
    let latest = 100;
    let mut store = Store::default();
    let mut index = Index::default();
    let document = |n: u32, time: i64| Put {
        doc: doc(n),
        commit: commit(time),
        source: None,
        text: Some(Text {
            name: format!("Note {n}"),
            body: "the same words".into(),
        }),
    };
    // Whoever wrote the index believed every time.
    apply(
        &mut index,
        &mut store,
        id(1),
        Batch {
            put: vec![document(1, i64::MAX / 2), document(2, 5)],
            ..Batch::default()
        },
    );
    let order = |latest: i64| -> Vec<DocId> {
        run(index.search(&store, &Query::parse("words "), 10, latest))
            .unwrap()
            .into_iter()
            .map(|hit| hit.doc)
            .collect()
    };
    assert_eq!(order(i64::MAX), [doc(1), doc(2)]);
    assert_eq!(order(latest), [doc(2), doc(1)]);

    let written = run(index.apply(
        &store,
        id(2),
        Batch {
            put: vec![put(1, 1, 50)],
            ..Batch::default()
        },
        latest,
    ))
    .unwrap();
    store.keep(&written);
    assert_eq!(
        found(&index, &store, "uniq1v1x "),
        BTreeSet::from([doc(1)]),
        "the honest put replaced the forged entry"
    );
    assert_eq!(
        run(index.entry(&store, &doc(1))).unwrap().unwrap().commit,
        commit(50)
    );
}
