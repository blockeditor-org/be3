use super::*;

#[test]
fn removals_of_unseen_documents_survive_a_merge() {
    let mut store = Store::default();
    let mut base = Index::default();
    apply(&mut base, &mut store, id(1), Batch { put: (0..10).map(|n| put(n, 0, 1)).collect(), ..Batch::default() });
    // One side removes a document and a block it never indexed.
    let mut removing = base.clone();
    apply(&mut removing, &mut store, id(2), Batch {
        removed: vec![(doc(900), commit(5))],
        removed_blocks: vec![(doc(901).block, commit(5))],
        ..Batch::default()
    });
    assert!(run(removing.entry(&store, &doc(900))).unwrap().unwrap().is_removed());
    // The other indexed both at older commits, and another at a newer one.
    let mut adding = base.clone();
    apply(&mut adding, &mut store, id(3), Batch {
        put: vec![put(900, 0, 3), put(901, 0, 3), put(2, 1, 6)],
        ..Batch::default()
    });
    for (a, b) in [(&removing, &adding), (&adding, &removing)] {
        let (merged, written) = run(a.merge(b, &store, id(4))).unwrap();
        store.keep(&written);
        assert!(found(&merged, &store, "mark900 ").is_empty());
        assert!(found(&merged, &store, "mark901 ").is_empty());
        assert_eq!(found(&merged, &store, "uniq2v1x "), [doc(2)].into());
        // A later batch on the merge indexes them anew at newer commits.
        let mut later = merged.clone();
        apply(&mut later, &mut store, id(5), Batch {
            put: vec![put(900, 1, 7), put(901, 1, 7)],
            ..Batch::default()
        });
        assert_eq!(found(&later, &store, "mark900 "), [doc(900)].into());
        assert_eq!(found(&later, &store, "mark901 "), [doc(901)].into());
    }
    // A put older than the block's removal is not indexed.
    let mut late = removing.clone();
    apply(&mut late, &mut store, id(6), Batch { put: vec![put(901, 0, 4)], ..Batch::default() });
    assert!(found(&late, &store, "mark901 ").is_empty());
}
