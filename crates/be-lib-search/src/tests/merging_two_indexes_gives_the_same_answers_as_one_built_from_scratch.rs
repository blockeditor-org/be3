use super::*;

#[test]
fn merging_two_indexes_gives_the_same_answers_as_one_built_from_scratch() {
    let mut store = Store::default();
    let mut base = Index::default();
    for batch in 0..6u32 {
        let put = (batch * 20..batch * 20 + 20)
            .map(|n| put(n, 0, 1))
            .collect();
        apply(
            &mut base,
            &mut store,
            id(batch),
            Batch {
                put,
                watermark: Some(10),
                ..Batch::default()
            },
        );
    }
    let mut documents: BTreeMap<u32, (u32, i64)> = (0..120).map(|n| (n, (0, 1))).collect();

    // One device adds documents, reindexes 3 and 4, and compacts.
    let mut a = base.clone();
    for batch in 0..5u32 {
        let mut put_list: Vec<Put> = (200 + batch * 10..210 + batch * 10)
            .map(|n| put(n, 0, 5))
            .collect();
        if batch == 0 {
            put_list.extend([put(3, 1, 10), put(4, 1, 30)]);
        }
        apply(
            &mut a,
            &mut store,
            id(100 + batch),
            Batch {
                put: put_list,
                watermark: Some(20),
                ..Batch::default()
            },
        );
    }
    store.keep(&run(a.compact(&store, id(150), 0, usize::MAX, u64::MAX, |_| 0)).unwrap());
    // The other adds others, reindexes 3 and 4 too, and removes 5 and 6.
    let mut b = base.clone();
    let put_list: Vec<Put> = (300..330)
        .map(|n| put(n, 0, 6))
        .chain([put(3, 2, 20), put(4, 2, 25)])
        .collect();
    apply(
        &mut b,
        &mut store,
        id(200),
        Batch {
            put: put_list,
            removed: vec![(doc(5), commit(7))],
            removed_blocks: vec![(doc(6).block, commit(7))],
            watermark: Some(15),
            at: 0,
        },
    );
    for n in (200..250).chain(300..330) {
        documents.insert(n, (0, if n < 300 { 5 } else { 6 }));
    }
    documents.insert(3, (2, 20));
    documents.insert(4, (1, 30));
    documents.remove(&5);
    documents.remove(&6);

    let (merged, written) = run(a.merge(&b, &store, id(300))).unwrap();
    store.keep(&written);
    let (reverse, written) = run(b.merge(&a, &store, id(301))).unwrap();
    store.keep(&written);
    assert_eq!(merged.watermark(), 15);
    let mut fresh_store = Store::default();
    let fresh = from_scratch(&documents, &mut fresh_store);
    same_answers((&merged, &store), (&fresh, &fresh_store), 330);
    same_answers((&reverse, &store), (&fresh, &fresh_store), 330);
    for n in [3, 4] {
        let version = documents[&n].0;
        assert_eq!(
            found(&merged, &store, &format!("uniq{n}v{version}x ")),
            [doc(n)].into()
        );
        for old in (0..3).filter(|old| *old != version) {
            assert!(found(&merged, &store, &format!("uniq{n}v{old}x ")).is_empty());
        }
    }
    // Merging again, and compacting the merge, changes no answer.
    let (again, written) = run(merged.merge(&reverse, &store, id(302))).unwrap();
    store.keep(&written);
    let mut compacted = again.clone();
    store.keep(&run(compacted.compact(&store, id(303), 0, usize::MAX, u64::MAX, |_| 0)).unwrap());
    same_answers((&again, &store), (&fresh, &fresh_store), 330);
    same_answers((&compacted, &store), (&fresh, &fresh_store), 330);
}
