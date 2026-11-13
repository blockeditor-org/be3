use super::*;

#[test]
fn a_long_run_of_batches_compacted_a_few_merges_at_a_time_stays_few_segments() {
    let mut store = Store::default();
    let mut index = Index::default();
    let mut documents: BTreeMap<u32, (u32, i64)> = BTreeMap::new();
    let mut most_segments = 0;
    // One document a batch, as ordinary edits write them, each compacted at most two merges
    // at a time, as a writer does.
    for n in 0..300u32 {
        let time = i64::from(n) + 1;
        apply(
            &mut index,
            &mut store,
            id(n),
            Batch {
                put: vec![put(n, 0, time)],
                ..Batch::default()
            },
        );
        documents.insert(n, (0, time));
        store.keep(&run(index.compact(&store, id(10_000 + n), time, 2, u64::MAX, |_| 0)).unwrap());
        most_segments = most_segments.max(index.segment_count());
    }
    // Tiers of four: three segments per power of four documents, and one batch's own.
    assert!(most_segments <= 3 * 5 + 1, "{most_segments} segments");
    let mut fresh_store = Store::default();
    let fresh = from_scratch(&documents, &mut fresh_store);
    same_answers((&index, &store), (&fresh, &fresh_store), 300);
}
