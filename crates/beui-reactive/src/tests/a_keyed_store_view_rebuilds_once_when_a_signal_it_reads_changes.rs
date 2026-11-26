use super::*;

#[test]
fn a_keyed_store_view_rebuilds_once_when_a_signal_it_reads_changes() {
    let scope = Scope::new();
    let store = OwnedKeyedStore::<u32, (u32, bool)>::new();
    store.reconcile_owned((0..5).map(|key| (key, (key, false))));
    let calls = Rc::new(Cell::new(0));
    let (threshold, archived, visible) = scope.run(|| {
        let threshold = create_signal(2);
        let archived = create_signal(false);
        let calls = calls.clone();
        let visible = store.filter(move |(value, hidden)| {
            calls.set(calls.get() + 1);
            *value >= threshold.get() && (!hidden || archived.get())
        });
        (threshold, archived, visible)
    });
    assert_eq!(visible.get_untracked(), [2, 3, 4]);

    calls.set(0);
    threshold.set(1);
    threshold.set(3);
    flush();
    assert_eq!(visible.get_untracked(), [3, 4]);
    assert_eq!(calls.get(), 5, "two writes rebuild the view once");

    store.update(&4, |(_, hidden)| *hidden = true);
    flush();
    assert_eq!(visible.get_untracked(), [3]);

    calls.set(0);
    archived.set(true);
    flush();
    assert_eq!(
        visible.get_untracked(),
        [3, 4],
        "a signal first read by an update is tracked too"
    );
    assert_eq!(calls.get(), 5);
}
