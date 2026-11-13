use super::*;

#[test]
fn a_removal_from_a_clock_far_behind_is_remembered_from_when_it_was_applied() {
    const DAY: i64 = 24 * 60 * 60 * 1000;
    let now = 400 * DAY;
    let mut store = Store::default();
    let mut base = Index::default();
    apply(
        &mut base,
        &mut store,
        id(1),
        Batch {
            put: vec![put(5, 0, 1), put(6, 0, 1)],
            ..Batch::default()
        },
    );

    // Block 5 is removed by a commit whose author's clock says a year ago, applied now.
    let mut removing = base.clone();
    apply(
        &mut removing,
        &mut store,
        id(2),
        Batch {
            removed_blocks: vec![(doc(5).block, commit(now - 365 * DAY))],
            at: now,
            ..Batch::default()
        },
    );
    // A compaction now forgets what is older than a week; the removal is a few seconds old here.
    store.keep(&run(removing.compact(&store, id(3), now, usize::MAX, u64::MAX, |_| 0)).unwrap());
    removing.forget_before(now - 7 * DAY);
    assert_eq!(
        removing.removed_blocks.len(),
        1,
        "the removal is remembered"
    );

    // A device that wrote on the base meanwhile still holds block 5's entry: the merge masks it.
    let mut other = base.clone();
    apply(
        &mut other,
        &mut store,
        id(4),
        Batch {
            put: vec![put(7, 0, 2)],
            ..Batch::default()
        },
    );
    for (origin, (a, b)) in [(5, (&removing, &other)), (6, (&other, &removing))] {
        let (merged, written) = run(a.merge(b, &store, id(origin))).unwrap();
        store.keep(&written);
        assert!(
            found(&merged, &store, "uniq5v0x ").is_empty(),
            "block 5 stays removed"
        );
        assert_eq!(found(&merged, &store, "uniq7v0x "), [doc(7)].into());
    }
}
