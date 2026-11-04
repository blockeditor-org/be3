use super::*;

#[test]
fn a_keyed_mapping_builds_a_row_for_each_repeat_of_a_key() {
    let scope = Scope::new();
    let builds = Rc::new(Cell::new(0));
    let items = scope.run({
        let builds = builds.clone();
        move || {
            KeyedItems::new(move |key: u32| {
                builds.set(builds.get() + 1);
                key * 10
            })
        }
    });

    assert_eq!(items.map(vec![1, 2, 1]).items(), [10, 20, 10]);
    assert_eq!(
        builds.get(),
        3,
        "each repeat of a key gets a row of its own"
    );

    assert_eq!(items.map(vec![1, 1, 2]).items(), [10, 10, 20]);
    assert_eq!(
        builds.get(),
        3,
        "a repeated key keeps its rows when the list is reordered"
    );

    assert_eq!(items.map(vec![1, 2]).items(), [10, 20]);
    assert_eq!(builds.get(), 3);
}
