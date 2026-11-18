use super::*;

#[test]
fn reordering_a_keyed_store_keeps_the_item_signals() {
    let scope = Scope::new();
    let store = OwnedKeyedStore::<u32, u32>::new();
    store.reconcile_owned([(0u32, 7u32), (1, 8)]);
    flush();
    let first = store.get(&0);
    let item_runs = Rc::new(Cell::new(0));
    let key_runs = Rc::new(Cell::new(0));
    let keys = store.keys();
    scope.run(|| {
        let runs = item_runs.clone();
        create_effect(move || {
            first.track();
            runs.set(runs.get() + 1);
        });
        let runs = key_runs.clone();
        create_effect(move || {
            keys.track();
            runs.set(runs.get() + 1);
        });
    });
    store.reconcile_owned([(1u32, 8u32), (0, 7)]);
    flush();
    assert_eq!(item_runs.get(), 1);
    assert_eq!(key_runs.get(), 2);
    assert_eq!(first, store.get(&0));
}
