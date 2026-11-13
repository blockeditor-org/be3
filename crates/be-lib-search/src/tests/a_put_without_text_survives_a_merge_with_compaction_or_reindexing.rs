use super::*;

/// Four segments of ten documents each, which compaction merges.
fn base(store: &mut Store) -> Index {
    let mut index = Index::default();
    for batch in 0..4 {
        let put = (batch * 10..batch * 10 + 10)
            .map(|n| Put {
                source: Some([9; 32]),
                ..put(n, 0, 1)
            })
            .collect();
        apply(
            &mut index,
            store,
            id(batch),
            Batch {
                put,
                ..Batch::default()
            },
        );
    }
    index
}

fn unchanged(n: u32, time: i64) -> Batch {
    Batch {
        put: vec![Put {
            doc: doc(n),
            commit: commit(time),
            source: Some([9; 32]),
            text: None,
            partial: false,
        }],
        ..Batch::default()
    }
}

#[test]
fn a_put_without_text_survives_a_merge_with_compaction_or_reindexing() {
    let mut store = Store::default();
    let base = base(&mut store);
    let mut moved = base.clone();
    apply(&mut moved, &mut store, id(10), unchanged(3, 5));

    // The other side compacted the segment the newer entry points into.
    let mut compacted = base.clone();
    store.keep(&run(compacted.compact(&store, id(20), 0, usize::MAX, u64::MAX, |_| 0)).unwrap());
    assert_eq!(compacted.segment_count(), 1);
    for (a, b) in [(&compacted, &moved), (&moved, &compacted)] {
        let (merged, written) = run(a.merge(b, &store, id(30))).unwrap();
        store.keep(&written);
        assert!(written.needs_text.is_empty());
        assert_eq!(found(&merged, &store, "uniq3v0x "), [doc(3)].into());
        let entry = run(merged.entry(&store, &doc(3))).unwrap().unwrap();
        assert_eq!((entry.commit, entry.indexed), (commit(5), commit(1)));
    }

    // The other side reindexed it at an older commit than the newer entry,
    // masking the place the newer entry keeps.
    let mut reindexed = base.clone();
    apply(
        &mut reindexed,
        &mut store,
        id(40),
        Batch {
            put: vec![put(3, 1, 4)],
            ..Batch::default()
        },
    );
    for (a, b) in [(&reindexed, &moved), (&moved, &reindexed)] {
        let (merged, written) = run(a.merge(b, &store, id(50))).unwrap();
        store.keep(&written);
        assert_eq!(found(&merged, &store, "uniq3v0x "), [doc(3)].into());
        assert!(found(&merged, &store, "uniq3v1x ").is_empty());
    }

    // Reindexed and compacted: no surviving segment holds the newer entry's
    // text, so the merge asks for it again.
    let mut both = reindexed.clone();
    store.keep(&run(both.compact(&store, id(60), 0, usize::MAX, u64::MAX, |_| 0)).unwrap());
    let (merged, written) = run(both.merge(&moved, &store, id(70))).unwrap();
    store.keep(&written);
    assert_eq!(written.needs_text, [doc(3)]);
    assert!(found(&merged, &store, "mark3 ").is_empty());
    assert_eq!(found(&merged, &store, "mark4 "), [doc(4)].into());

    // A put without text for a document the index has nothing for.
    let mut index = base.clone();
    let written = run(index.apply(&store, id(80), unchanged(77, 5), i64::MAX)).unwrap();
    assert_eq!(written.needs_text, [doc(77)]);
}
