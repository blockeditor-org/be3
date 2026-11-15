use super::*;
use crate::reactive::{ForEach, ForEachRef, flush};

#[test]
fn a_for_each_handle_knows_a_repeated_key_by_its_first_place_and_holds_every_repeat() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let keys = create_signal(vec![1_u32, 2, 1]);
    let rows = scope.run(ForEachRef::<u32, DomNode>::new);
    let _page = scope.run(|| {
        view! {
            <Element tag="ul">
                <ForEach value={keys} handle={rows}>|key: u32|
                    <Element tag="b" text={key.to_string()} />
                </ForEach>
            </Element>
        }
    });
    assert_eq!(*rows.keys(), [1, 2, 1]);
    assert_eq!(rows.index_of(&1), Some(0), "a repeated key is known by its first place");
    let index = rows.index(&1).expect("a shown key has a row");
    assert_eq!(index.get_untracked(), 0, "and its first row's index");
    assert_eq!(rows.nodes_of(&1).len(), 2, "its nodes are every repeat's, in order");
    let mut built = Vec::new();
    rows.for_each_row(|row| built.push((row.key, row.index.get_untracked())));
    assert_eq!(built, [(1, 0), (2, 1), (1, 2)], "the built rows are walked in order");

    keys.set(vec![2, 1]);
    flush();
    assert_eq!(rows.index_of(&1), Some(1));
    assert_eq!(rows.nodes_of(&1).len(), 1);
}
