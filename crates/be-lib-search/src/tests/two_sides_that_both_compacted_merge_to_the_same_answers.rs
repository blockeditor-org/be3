use super::*;

/// Two devices write one batch each onto the same base and both compact, as each device that
/// writes an index does (be-host-stack's `search::write`): merged either way, the index answers
/// as one built from scratch.
#[test]
fn two_sides_that_both_compacted_merge_to_the_same_answers() {
    let mut store = Store::default();
    let mut base = Index::default();
    for batch in 0..15u32 {
        apply(
            &mut base,
            &mut store,
            id(batch),
            Batch {
                put: vec![put(batch, 0, 1)],
                ..Batch::default()
            },
        );
        store.keep(&run(base.compact(&store, id(1000 + batch))).unwrap());
    }
    let mut documents: BTreeMap<u32, (u32, i64)> = (0..15).map(|n| (n, (0, 1))).collect();

    let mut a = base.clone();
    apply(
        &mut a,
        &mut store,
        id(100),
        Batch {
            put: vec![put(15, 0, 2), put(3, 1, 3)],
            ..Batch::default()
        },
    );
    store.keep(&run(a.compact(&store, id(150))).unwrap());
    let mut b = base.clone();
    apply(
        &mut b,
        &mut store,
        id(200),
        Batch {
            put: vec![put(16, 0, 2), put(4, 1, 3)],
            ..Batch::default()
        },
    );
    store.keep(&run(b.compact(&store, id(250))).unwrap());
    assert!(
        a.segment_count() < base.segment_count() && b.segment_count() < base.segment_count(),
        "both sides compacted"
    );
    for (n, version) in [(15, 0), (16, 0), (3, 1), (4, 1)] {
        documents.insert(n, (version, if version == 0 { 2 } else { 3 }));
    }

    let (merged, written) = run(a.merge(&b, &store, id(300))).unwrap();
    store.keep(&written);
    let (reverse, written) = run(b.merge(&a, &store, id(301))).unwrap();
    store.keep(&written);
    let mut fresh_store = Store::default();
    let fresh = from_scratch(&documents, &mut fresh_store);
    same_answers((&merged, &store), (&fresh, &fresh_store), 17);
    same_answers((&reverse, &store), (&fresh, &fresh_store), 17);
    let mut compacted = merged.clone();
    store.keep(&run(compacted.compact(&store, id(302))).unwrap());
    same_answers((&compacted, &store), (&fresh, &fresh_store), 17);
}
