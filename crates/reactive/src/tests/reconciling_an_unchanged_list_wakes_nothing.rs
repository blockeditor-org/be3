use super::*;

#[test]
fn reconciling_an_unchanged_list_wakes_nothing() {
    let scope = Scope::new();
    let store: KeyedStore<u32, String> = KeyedStore::new();
    let items: Vec<(u32, String)> = (0..3).map(|index| (index, index.to_string())).collect();
    store.reconcile(items.iter().map(|(key, value)| (*key, value)));
    let runs = Cell::new(0);
    let runs = Rc::new(runs);
    let keys = store.keys();
    scope.run(|| {
        let item = store.get(&1);
        let runs = runs.clone();
        let keys = keys.clone();
        create_effect(move || {
            keys.with(|keys| assert_eq!(keys.len(), 3));
            item.with(|_| ());
            runs.set(runs.get() + 1);
        });
    });
    assert_eq!(runs.get(), 1);
    store.reconcile(items.iter().map(|(key, value)| (*key, value)));
    assert_eq!(runs.get(), 1);
}
