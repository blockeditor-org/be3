use super::*;

#[test]
fn a_keyed_store_filters_its_keys_waking_only_when_one_joins_or_leaves() {
    let scope = Scope::new();
    let store = OwnedKeyedStore::<u32, (bool, u32)>::new();
    store.reconcile_owned((0..6).map(|key| (key, (key % 2 == 0, 0))));
    let calls = Rc::new(Cell::new(0));
    let runs = Rc::new(Cell::new(0));
    let done = scope.run(|| {
        let calls = calls.clone();
        let done = store.filter(move |(done, _)| {
            calls.set(calls.get() + 1);
            *done
        });
        let runs = runs.clone();
        create_effect(move || {
            done.with(|_| ());
            runs.set(runs.get() + 1);
        });
        done
    });
    assert_eq!(done.get_untracked(), [0, 2, 4]);

    calls.set(0);
    store.update(&3, |(done, _)| *done = true);
    flush();
    assert_eq!(
        done.get_untracked(),
        [0, 2, 3, 4],
        "a key joins in store order"
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(runs.get(), 2);

    store.update(&3, |(_, edits)| *edits += 1);
    store.update(&1, |(_, edits)| *edits += 1);
    flush();
    assert_eq!(runs.get(), 2, "edits that keep the result wake nothing");

    store.update(&2, |(done, _)| *done = false);
    store.set(6, (true, 0));
    store.remove(&0);
    flush();
    assert_eq!(done.get_untracked(), [3, 4, 6]);

    calls.set(0);
    store.reconcile_owned([
        (6, (true, 0)),
        (5, (true, 0)),
        (4, (true, 0)),
        (3, (true, 1)),
    ]);
    flush();
    assert_eq!(
        done.get_untracked(),
        [6, 5, 4, 3],
        "a reorder follows the store"
    );
    assert_eq!(calls.get(), 1, "only the new value of 5 is read");

    store.reconcile_owned([
        (6, (true, 0)),
        (7, (true, 0)),
        (5, (true, 0)),
        (4, (true, 0)),
        (3, (true, 1)),
    ]);
    assert_eq!(
        done.get_untracked(),
        [6, 7, 5, 4, 3],
        "a key reconciled into the middle lands there"
    );
}
