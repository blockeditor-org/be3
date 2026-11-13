use super::*;

#[test]
fn a_merge_over_the_byte_cap_is_not_made() {
    let mut store = Store::default();
    let mut index = Index::default();
    for n in 0..4u32 {
        apply(
            &mut index,
            &mut store,
            id(n),
            Batch {
                put: vec![put(n, 0, 1)],
                ..Batch::default()
            },
        );
    }
    assert_eq!(index.segment_count(), 4);
    let bytes = |part: &PartName| {
        store
            .parts
            .parts
            .get(part)
            .map_or(0, |bytes| bytes.len() as u64)
    };
    let total: u64 = index.parts().iter().map(bytes).sum();

    // Four segments of a tier would merge, but they come to more than the cap: left as they are.
    let written = run(index.compact(&store, id(10), 1, usize::MAX, total / 2, bytes)).unwrap();
    assert!(written.added.is_empty());
    assert_eq!(index.segment_count(), 4);

    // With room for them, they merge.
    let written = run(index.compact(&store, id(11), 1, usize::MAX, total, bytes)).unwrap();
    store.keep(&written);
    assert_eq!(index.segment_count(), 1);
    assert_eq!(found(&index, &store, "uniq2v0x "), [doc(2)].into());
}
