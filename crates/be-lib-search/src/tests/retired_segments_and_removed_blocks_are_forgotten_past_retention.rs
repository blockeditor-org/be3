use super::*;

#[test]
fn retired_segments_and_removed_blocks_are_forgotten_past_retention() {
    let mut store = Store::default();
    let mut index = Index::default();
    for batch in 0..4u32 {
        apply(
            &mut index,
            &mut store,
            id(batch),
            Batch {
                put: vec![put(batch, 0, 1)],
                ..Batch::default()
            },
        );
    }
    // A block removed by a commit at 5, and segments retired by a compaction at 7.
    apply(
        &mut index,
        &mut store,
        id(10),
        Batch {
            removed_blocks: vec![(doc(0).block, commit(5))],
            ..Batch::default()
        },
    );
    store.keep(&run(index.compact(&store, id(11), 7, usize::MAX, u64::MAX, |_| 0)).unwrap());
    assert_eq!(index.removed_blocks.len(), 1);
    assert!(!index.retired.is_empty());

    // Kept while a merge with an index from before them could still bring them back.
    index.forget_before(5);
    assert_eq!(index.removed_blocks.len(), 1);
    index.forget_before(6);
    assert!(
        index.removed_blocks.is_empty(),
        "the removed block is forgotten"
    );
    assert!(!index.retired.is_empty());
    index.forget_before(8);
    assert!(
        index.retired.is_empty(),
        "the retired segments are forgotten"
    );
    assert_eq!(index.remembered(), 0);
    assert!(found(&index, &store, "uniq0v0x ").is_empty());
    assert_eq!(found(&index, &store, "uniq1v0x "), [doc(1)].into());
}
