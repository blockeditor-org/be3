use super::*;
use crate::reactive::{ForEach, ForEachRef, Show, flush};

#[test]
fn a_for_each_handle_follows_each_key_across_a_reorder_an_insert_and_a_removal() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let keys = create_signal(vec![1_u32, 2, 3]);
    let marked = create_signal(false);
    let rows = scope.run(ForEachRef::<u32, DomNode>::new);
    let _page = scope.run(|| {
        view! {
            <Element tag="ul">
                <ForEach value={keys} handle={rows}>|key: u32|
                    <Element tag="b" text={key.to_string()} />
                    <Show value={marked}>
                        <Element tag="i" text={format!("{key}!")} />
                    </Show>
                </ForEach>
            </Element>
        }
    });
    let html = |key: u32| {
        rows.children_of(&key)
            .iter()
            .map(DomNode::html)
            .collect::<Vec<_>>()
    };
    assert_eq!(rows.keys(), vec![1, 2, 3]);
    assert_eq!(rows.index_of(&3), Some(2));
    let first = html(1);
    assert_eq!(first.len(), 1, "a row of one child holds one");

    keys.set(vec![3, 1, 2]);
    flush();
    assert_eq!(rows.keys(), vec![3, 1, 2]);
    assert_eq!(rows.index_of(&1), Some(1), "a key's place follows it");
    assert_eq!(html(1), first, "and so do the children its row built, unbuilt again");
    let index = rows.index(&1).expect("a shown key has a place");

    keys.set(vec![3, 4, 1, 2]);
    flush();
    assert_eq!(index.get_untracked(), 2, "an insert above moves the place a row was handed");
    assert_eq!(rows.index_of(&4), Some(1));

    marked.set(true);
    flush();
    assert_eq!(html(1).len(), 2, "a row whose run grows holds its whole run");
    let shown = rows.children_of(&4)[0].clone();
    assert_eq!(rows.key_of(&shown), Some(4));

    keys.set(vec![3, 1, 2]);
    flush();
    assert!(rows.children_of(&4).is_empty(), "a key removed holds nothing");
    assert_eq!(rows.index_of(&4), None);
    assert_eq!(rows.key_of(&shown), None);
}
