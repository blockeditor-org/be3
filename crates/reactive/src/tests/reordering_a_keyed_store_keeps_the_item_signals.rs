use super::*;

#[test]
fn reordering_a_keyed_store_keeps_the_item_signals() {
    let scope = Scope::new();
    let store: KeyedStore<u32, u32> = KeyedStore::new();
    store.reconcile_owned([(0u32, 7u32), (1, 8)]);
    let first = store.get(&0);
    let item_runs = Rc::new(Cell::new(0));
    let key_runs = Rc::new(Cell::new(0));
    let keys = store.keys();
    scope.run(|| {
        let runs = item_runs.clone();
        let first = first.clone();
        create_effect(move || {
            first.with(|_| ());
            runs.set(runs.get() + 1);
        });
        let runs = key_runs.clone();
        create_effect(move || {
            keys.with(|_| ());
            runs.set(runs.get() + 1);
        });
    });
    store.reconcile_owned([(1u32, 8u32), (0, 7)]);
    assert_eq!(item_runs.get(), 1);
    assert_eq!(key_runs.get(), 2);
    assert!(first.ptr_eq(&store.get(&0)));
}
