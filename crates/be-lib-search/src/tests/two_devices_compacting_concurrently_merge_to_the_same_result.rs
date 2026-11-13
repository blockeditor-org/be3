use super::*;

#[test]
fn two_devices_compacting_concurrently_merge_to_the_same_result() {
    let mut store = Store::default();
    let documents: BTreeMap<u32, (u32, i64)> = (0..40).map(|n| (n, (0, 1))).collect();
    let mut base = Index::default();
    for (n, &(version, time)) in &documents {
        apply(
            &mut base,
            &mut store,
            id(*n),
            Batch {
                put: vec![put(*n, version, time)],
                ..Batch::default()
            },
        );
    }

    // Each device writes its own batches on the same base and compacts with each, a few merges
    // at a time.
    let mut expected = documents.clone();
    let mut a = base.clone();
    for n in 100..120u32 {
        let time = i64::from(n);
        apply(
            &mut a,
            &mut store,
            id(n),
            Batch {
                put: vec![put(n, 0, time), put(n % 40, 1, time)],
                ..Batch::default()
            },
        );
        expected.insert(n, (0, time));
        expected.insert(n % 40, (1, time));
        store.keep(&run(a.compact(&store, id(1000 + n), time, 2, u64::MAX, |_| 0)).unwrap());
    }
    let mut b = base.clone();
    for n in 200..220u32 {
        let time = i64::from(n);
        let removed = n % 40 + 1;
        apply(
            &mut b,
            &mut store,
            id(n),
            Batch {
                put: vec![put(n, 0, time)],
                removed: vec![(doc(removed), commit(time))],
                ..Batch::default()
            },
        );
        expected.insert(n, (0, time));
        expected.remove(&removed);
        store.keep(&run(b.compact(&store, id(2000 + n), time, 2, u64::MAX, |_| 0)).unwrap());
    }

    // Merged either way, and compacted again, they answer as one index built from scratch.
    let (ab, written) = run(a.merge(&b, &store, id(3000))).unwrap();
    store.keep(&written);
    let (mut ba, written) = run(b.merge(&a, &store, id(3001))).unwrap();
    store.keep(&written);
    store.keep(&run(ba.compact(&store, id(3002), 300, usize::MAX, u64::MAX, |_| 0)).unwrap());
    let mut fresh_store = Store::default();
    let fresh = from_scratch(&expected, &mut fresh_store);
    same_answers((&ab, &store), (&fresh, &fresh_store), 220);
    same_answers((&ba, &store), (&fresh, &fresh_store), 220);
}
