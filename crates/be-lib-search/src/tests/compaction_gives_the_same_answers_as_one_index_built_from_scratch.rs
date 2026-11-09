use super::*;

#[test]
fn compaction_gives_the_same_answers_as_one_index_built_from_scratch() {
    let mut store = Store::default();
    let mut index = Index::default();
    let mut documents: BTreeMap<u32, (u32, i64)> = BTreeMap::new();
    let mut most_segments = 0;
    for batch in 0..40u32 {
        let time = i64::from(batch) + 1;
        let mut put_list: Vec<Put> = (batch * 8..batch * 8 + 8)
            .map(|n| put(n, 0, time))
            .collect();
        // Reindex an older document, and remove another.
        let reindexed = (batch * 13) % (batch * 8).max(1);
        if batch >= 2
            && let Some(&(version, _)) = documents.get(&reindexed)
        {
            put_list.push(put(reindexed, version + 1, time));
            documents.insert(reindexed, (version + 1, time));
        }
        let mut removed = Vec::new();
        if batch % 5 == 4 && batch * 3 != reindexed {
            removed.push((doc(batch * 3), commit(time)));
            documents.remove(&(batch * 3));
        }
        for n in batch * 8..batch * 8 + 8 {
            documents.insert(n, (0, time));
        }
        apply(
            &mut index,
            &mut store,
            id(batch),
            Batch {
                put: put_list,
                removed,
                ..Batch::default()
            },
        );
        let written = run(index.compact(&store, id(1000 + batch))).unwrap();
        store.keep(&written);
        most_segments = most_segments.max(index.segment_count());
    }
    // Tiers of four: at most three segments per power of four documents.
    assert!(most_segments <= 3 * 5, "{most_segments} segments");
    let mut fresh_store = Store::default();
    let fresh = from_scratch(&documents, &mut fresh_store);
    same_answers((&index, &store), (&fresh, &fresh_store), 320);
    for n in (0..320).step_by(3) {
        let expected = documents.contains_key(&n);
        assert_eq!(
            found(&index, &store, &format!("\"mark{n} sign{n}\"")).contains(&doc(n)),
            expected
        );
    }
}
