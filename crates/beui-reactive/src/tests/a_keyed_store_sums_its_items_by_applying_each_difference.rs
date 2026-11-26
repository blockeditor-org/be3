use super::*;

#[derive(Clone, PartialEq)]
struct Counter {
    count: i64,
    label: &'static str,
}

fn counter(count: i64) -> Counter {
    Counter {
        count,
        label: "counter",
    }
}

#[test]
fn a_keyed_store_sums_its_items_by_applying_each_difference() {
    let scope = Scope::new();
    let store = OwnedKeyedStore::<u32, Counter>::new();
    store.reconcile_owned((0..4).map(|key| (key, counter(i64::from(key)))));
    let calls = Rc::new(Cell::new(0));
    let seen = Rc::new(RefCell::new(Vec::new()));
    let total = scope.run(|| {
        let calls = calls.clone();
        let total = store.sum_by(move |counter| {
            calls.set(calls.get() + 1);
            counter.count
        });
        let seen = seen.clone();
        create_effect(move || seen.borrow_mut().push(total.get()));
        total
    });
    assert_eq!(total.get_untracked(), 6);
    assert_eq!(calls.get(), 4, "built once from every item");

    calls.set(0);
    store.update(&2, |counter| counter.count = 10);
    flush();
    assert_eq!(total.get_untracked(), 14);
    assert_eq!(calls.get(), 2, "an update reads only the old and new value");

    calls.set(0);
    store.update(&2, |counter| counter.label = "renamed");
    flush();
    assert_eq!(*seen.borrow(), [6, 14], "an unrelated field wakes nothing");

    store.set(9, counter(100));
    store.remove(&0);
    store.remove(&3);
    flush();
    assert_eq!(total.get_untracked(), 1 + 10 + 100);

    store.reconcile_owned([(9, counter(1)), (1, counter(2)), (5, counter(3))]);
    flush();
    assert_eq!(total.get_untracked(), 6);
    assert_eq!(calls.get(), 2 + 1 + 1 + 1 + 2 + 2 + 1 + 1);
    assert_eq!(*seen.borrow(), [6, 14, 111, 6]);
}
