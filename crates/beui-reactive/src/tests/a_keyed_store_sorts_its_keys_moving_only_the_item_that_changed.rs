use super::*;

#[derive(Clone, PartialEq)]
struct Task {
    rank: u32,
    note: &'static str,
}

fn task(rank: u32) -> Task {
    Task { rank, note: "" }
}

#[test]
fn a_keyed_store_sorts_its_keys_moving_only_the_item_that_changed() {
    let scope = Scope::new();
    let store = OwnedKeyedStore::<char, Task>::new();
    store.reconcile_owned([
        ('a', task(3)),
        ('b', task(1)),
        ('c', task(3)),
        ('d', task(2)),
    ]);
    let calls = Rc::new(Cell::new(0));
    let runs = Rc::new(Cell::new(0));
    let sorted = scope.run(|| {
        let calls = calls.clone();
        let sorted = store.sorted_by_key(move |task| {
            calls.set(calls.get() + 1);
            task.rank
        });
        let runs = runs.clone();
        create_effect(move || {
            sorted.with(|_| ());
            runs.set(runs.get() + 1);
        });
        sorted
    });
    assert_eq!(
        sorted.get_untracked(),
        ['b', 'd', 'a', 'c'],
        "ties keep store order"
    );
    assert_eq!(calls.get(), 4);

    calls.set(0);
    store.update(&'c', |task| task.rank = 0);
    flush();
    assert_eq!(sorted.get_untracked(), ['c', 'b', 'd', 'a']);
    assert_eq!(calls.get(), 1, "an update reads only the new value");
    assert_eq!(runs.get(), 2);

    store.update(&'a', |task| task.note = "edited");
    store.update(&'d', |task| task.rank = 2);
    store.update(&'b', |task| task.rank = 2);
    flush();
    assert_eq!(
        sorted.get_untracked(),
        ['c', 'b', 'd', 'a'],
        "b ties with d and stays before it"
    );
    assert_eq!(runs.get(), 2, "nothing moved, so nothing woke");

    store.update(&'b', |task| task.rank = 9);
    store.set('e', task(2));
    store.remove(&'c');
    flush();
    assert_eq!(sorted.get_untracked(), ['d', 'e', 'a', 'b']);

    calls.set(0);
    let edited = Task {
        rank: 3,
        note: "edited",
    };
    store.reconcile_owned([
        ('e', task(2)),
        ('a', edited),
        ('d', task(2)),
        ('b', task(9)),
    ]);
    flush();
    assert_eq!(
        sorted.get_untracked(),
        ['e', 'd', 'a', 'b'],
        "a reorder changes how ties break"
    );
    assert_eq!(calls.get(), 0, "a reorder reads no item");
}
