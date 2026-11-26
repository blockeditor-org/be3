use super::*;

#[test]
fn a_merge_drops_a_document_the_table_moved_though_unmasked() {
    let mut store = Store::default();
    let mut index = Index::default();
    for (batch, puts) in [
        vec![put(0, 0, 1), put(1, 0, 1)],
        vec![put(0, 1, 2)],
        vec![put(2, 0, 3)],
        vec![put(3, 0, 4)],
    ]
    .into_iter()
    .enumerate()
    {
        apply(
            &mut index,
            &mut store,
            id(batch as u32),
            Batch {
                put: puts,
                ..Batch::default()
            },
        );
    }
    assert_eq!(index.segment_count(), 4);

    // The first segment's deletions forgotten: only the table says document 0 moved on.
    let deletions = index.segments[0]
        .deletions
        .expect("document 0's first version is masked");
    store.parts.parts.get_mut().insert(deletions, Vec::new());

    let written =
        run(index.compact(&store, &store, id(50), 0, usize::MAX, u64::MAX, |_| 0)).unwrap();
    store.keep(&written);
    assert_eq!(index.segment_count(), 1);
    assert_eq!(index.segments[0].docs, 4);
    assert!(found(&index, &store, "uniq0v0x ").is_empty());
    assert_eq!(found(&index, &store, "uniq0v1x "), [doc(0)].into());
}
