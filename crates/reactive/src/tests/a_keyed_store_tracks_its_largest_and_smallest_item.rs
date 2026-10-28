use super::*;

#[test]
fn a_keyed_store_tracks_its_largest_and_smallest_item() {
    let scope = Scope::new();
    let store = OwnedKeyedStore::<u32, (u32, bool)>::new();
    store.reconcile_owned([
        (0, (5, false)),
        (1, (9, false)),
        (2, (9, false)),
        (3, (1, false)),
    ]);
    let runs = Rc::new(Cell::new(0));
    let (largest, smallest) = scope.run(|| {
        let largest = store.max_by(|(value, _)| *value);
        let smallest = store.min_by(|(value, _)| *value);
        let runs = runs.clone();
        create_effect(move || {
            largest.with(|_| ());
            runs.set(runs.get() + 1);
        });
        (largest, smallest)
    });
    assert_eq!(largest.get_untracked(), Some(9));
    assert_eq!(smallest.get_untracked(), Some(1));

    store.remove(&1);
    flush();
    assert_eq!(
        largest.get_untracked(),
        Some(9),
        "another item still holds 9"
    );
    assert_eq!(runs.get(), 1);

    store.update(&2, |(value, _)| *value = 7);
    flush();
    assert_eq!(largest.get_untracked(), Some(7));
    assert_eq!(runs.get(), 2);

    store.update(&2, |(_, flag)| *flag = true);
    store.update(&0, |(value, _)| *value = 6);
    flush();
    assert_eq!(runs.get(), 2, "changes below the largest item wake nothing");

    store.reconcile_owned([(3, (1, false)), (4, (0, false))]);
    flush();
    assert_eq!(largest.get_untracked(), Some(1));
    assert_eq!(smallest.get_untracked(), Some(0));

    store.reconcile_owned([]);
    assert_eq!(largest.get_untracked(), None);
    assert_eq!(smallest.get_untracked(), None);
}
