use super::*;

#[test]
fn a_keyed_mapping_builds_reuses_and_disposes_its_items() {
    let scope = Scope::new();
    let builds = Rc::new(Cell::new(0));
    let disposals = Rc::new(Cell::new(0));
    let items = scope.run({
        let (builds, disposals) = (builds.clone(), disposals.clone());
        move || {
            KeyedItems::new(move |key: u32| {
                builds.set(builds.get() + 1);
                let disposals = disposals.clone();
                on_cleanup(move || disposals.set(disposals.get() + 1));
                key * 10
            })
        }
    });

    assert_eq!(items.map(vec![1, 2]).items(), [10, 20]);
    assert_eq!(builds.get(), 2);
    assert_eq!(disposals.get(), 0);

    assert_eq!(
        items.map(vec![2, 1]).items(),
        [20, 10],
        "a key that stayed must keep the item that was built for it"
    );
    assert_eq!(builds.get(), 2);
    assert_eq!(disposals.get(), 0);

    assert_eq!(items.map(vec![2, 3]).items(), [20, 30]);
    assert_eq!(builds.get(), 3);
    assert_eq!(
        disposals.get(),
        1,
        "a key that left must dispose the scope its item was built in"
    );

    drop(items);
    assert_eq!(
        disposals.get(),
        3,
        "dropping the mapping disposes every item it still holds"
    );
}
