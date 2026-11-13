use super::*;

#[test]
fn a_reindexed_document_no_longer_matches_its_old_words() {
    let mut store = Store::default();
    let mut index = Index::default();
    let first = (0..40).map(|n| put(n, 0, 1)).collect();
    apply(
        &mut index,
        &mut store,
        id(1),
        Batch {
            put: first,
            ..Batch::default()
        },
    );
    apply(
        &mut index,
        &mut store,
        id(2),
        Batch {
            put: vec![put(7, 1, 2), put(8, 5, 0)],
            removed: vec![(doc(9), commit(2))],
            removed_blocks: vec![(doc(10).block, commit(2))],
            watermark: Some(12),
            at: 0,
        },
    );
    assert_eq!(index.watermark(), 12);
    assert!(found(&index, &store, "uniq7v0x ").is_empty());
    assert_eq!(found(&index, &store, "uniq7v1x "), [doc(7)].into());
    assert_eq!(found(&index, &store, "\"mark7 sign7\""), [doc(7)].into());
    // An older commit than the indexed one changes nothing.
    assert_eq!(found(&index, &store, "uniq8v0x "), [doc(8)].into());
    assert!(found(&index, &store, "uniq8v5x ").is_empty());
    assert!(found(&index, &store, "mark9").is_empty());
    assert!(found(&index, &store, "mark10").is_empty());
    let entry = run(index.entry(&store, &doc(9))).unwrap().unwrap();
    assert!(entry.is_removed());
    assert_eq!(entry.commit, commit(2));

    // Unchanged text from the same part moves only the commit; from another part, or from
    // no part, it is asked for again.
    let source = Some([7; 32]);
    apply(
        &mut index,
        &mut store,
        id(3),
        Batch {
            put: vec![crate::Put {
                source,
                ..put(7, 1, 3)
            }],
            ..Batch::default()
        },
    );
    let elsewhere = run(index.apply(
        &store,
        id(4),
        Batch {
            put: vec![crate::Put {
                doc: doc(7),
                commit: commit(4),
                source: Some([8; 32]),
                text: None,
                partial: false,
            }],
            ..Batch::default()
        },
    ))
    .unwrap();
    store.keep(&elsewhere);
    assert_eq!(elsewhere.needs_text, [doc(7)]);
    apply(
        &mut index,
        &mut store,
        id(5),
        Batch {
            put: vec![crate::Put {
                doc: doc(7),
                commit: commit(5),
                source,
                text: None,
                partial: false,
            }],
            ..Batch::default()
        },
    );
    let entry = run(index.entry(&store, &doc(7))).unwrap().unwrap();
    assert_eq!((entry.commit, entry.source), (commit(5), source));
    assert_eq!(found(&index, &store, "uniq7v1x "), [doc(7)].into());
    assert_eq!(
        run(index.entries_of(&store, &doc(7).block)).unwrap(),
        [entry]
    );
}
