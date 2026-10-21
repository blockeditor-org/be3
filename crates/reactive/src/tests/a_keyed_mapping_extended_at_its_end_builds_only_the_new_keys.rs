use super::*;

#[test]
fn a_keyed_mapping_extended_at_its_end_builds_only_the_new_keys() {
    let scope = Scope::new();
    let built = Rc::new(RefCell::new(Vec::new()));
    let items = scope.run({
        let built = built.clone();
        move || {
            KeyedItems::new(move |key: u32| {
                built.borrow_mut().push(key);
                key * 10
            })
        }
    });

    assert_eq!(items.map(vec![1, 2, 1]).items(), [10, 20, 10]);
    assert_eq!(items.map(vec![1, 2, 1, 3, 4]).items(), [10, 20, 10, 30, 40]);
    assert_eq!(*built.borrow(), [1, 2, 1, 3, 4]);

    assert_eq!(
        items.map(vec![1, 2, 1, 3, 4, 2]).items(),
        [10, 20, 10, 30, 40, 20],
        "a key repeated at the end gets a row of its own"
    );
    assert_eq!(*built.borrow(), [1, 2, 1, 3, 4, 2]);

    assert_eq!(items.map(vec![4, 1]).items(), [40, 10]);
    assert_eq!(items.map(vec![4, 1, 5]).items(), [40, 10, 50]);
    assert_eq!(
        *built.borrow(),
        [1, 2, 1, 3, 4, 2, 5],
        "keys kept through a reorder are not built again when more are added"
    );
}
