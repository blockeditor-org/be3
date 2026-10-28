use super::*;

#[test]
fn a_keyed_store_updates_one_item_in_place() {
    let scope = Scope::new();
    let store = OwnedKeyedStore::<u32, (u32, String)>::new();
    store.set(1, (0, "one".to_owned()));
    let runs = Rc::new(Cell::new(0));
    scope.run(|| {
        let item = store.get(&1);
        let runs = runs.clone();
        create_effect(move || {
            item.with(|_| ());
            runs.set(runs.get() + 1);
        });
    });
    assert_eq!(runs.get(), 1);

    let returned = store.update(&1, |(count, _)| {
        *count += 1;
        *count
    });
    flush();
    assert_eq!(returned, Some(1), "update returns what its closure returns");
    assert_eq!(store.get(&1).get_untracked(), (1, "one".to_owned()));
    assert_eq!(runs.get(), 2);

    store.update(&1, |(_, label)| label.make_ascii_lowercase());
    flush();
    assert_eq!(
        runs.get(),
        2,
        "an update that changes nothing wakes nothing"
    );

    assert_eq!(
        store.update(&2, |(count, _)| *count += 1),
        None,
        "a missing key is not inserted"
    );
    assert!(!store.contains(&2));
}
