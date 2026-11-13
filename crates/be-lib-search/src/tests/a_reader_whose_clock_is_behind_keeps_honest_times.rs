use super::*;

/// An index stores commit times as given and only compares them as believed
/// (`Commit::believed`), so a device whose clock is far behind still applies a newer put over
/// an older one and leaves the honest times for readers whose clocks are right.
#[test]
fn a_reader_whose_clock_is_behind_keeps_honest_times() {
    let mut store = Store::default();
    let mut index = Index::default();
    apply(
        &mut index,
        &mut store,
        id(1),
        Batch {
            put: vec![put(1, 0, 1_000)],
            ..Batch::default()
        },
    );
    let behind = 10;
    let written = run(index.apply(
        &store,
        id(2),
        Batch {
            put: vec![put(1, 1, 1_100), put(2, 0, 1_200)],
            ..Batch::default()
        },
        behind,
    ))
    .unwrap();
    store.keep(&written);
    assert_eq!(
        run(index.entry(&store, &doc(2))).unwrap().unwrap().commit,
        commit(1_200),
        "the time was stored as believed"
    );
    // Every time the slow reader saw counts as the oldest, so it compared the two puts of
    // document 1 by id alone; a reader whose clock is right still finds both documents.
    assert_eq!(found(&index, &store, "uniq2v0x "), BTreeSet::from([doc(2)]));
}
