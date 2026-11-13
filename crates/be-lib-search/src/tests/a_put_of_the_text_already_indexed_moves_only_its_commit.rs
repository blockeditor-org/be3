use super::*;

#[test]
fn a_put_of_the_text_already_indexed_moves_only_its_commit() {
    let mut store = Store::default();
    let mut index = Index::default();
    apply(
        &mut index,
        &mut store,
        id(1),
        Batch {
            put: (0..3000).map(|n| put(n, 0, 1)).collect(),
            ..Batch::default()
        },
    );
    let segments = index.segment_count();

    // Every row put again at a newer commit, one of them changed: only that one is new text.
    let mut again: Vec<Put> = (0..3000).map(|n| put(n, 0, 2)).collect();
    again[7] = put(7, 1, 2);
    let written = run(index.apply(
        &store,
        id(2),
        Batch {
            put: again,
            ..Batch::default()
        },
    ))
    .unwrap();
    store.keep(&written);
    assert_eq!(
        index.segment_count(),
        segments + 1,
        "one segment, for the changed row"
    );
    let entry = run(index.entry(&store, &doc(8))).unwrap().unwrap();
    assert_eq!(
        entry.commit,
        commit(2),
        "an unchanged row moves to the new commit"
    );
    assert_eq!(
        entry.indexed,
        commit(1),
        "and keeps the text it was indexed with"
    );
    assert_eq!(found(&index, &store, "uniq8v0x "), [doc(8)].into());
    assert_eq!(found(&index, &store, "uniq7v1x "), [doc(7)].into());
    assert!(found(&index, &store, "uniq7v0x ").is_empty());
}
