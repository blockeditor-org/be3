use super::*;
use crate::reactive::{ForEach, ForEachRef};

#[test]
fn a_for_each_handle_is_empty_once_its_list_is_gone() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let rows = ForEachRef::<u32, DomNode>::new();
    let list = Scope::new();
    let _page = list.run(|| {
        view! {
            <Element tag="ul">
                <ForEach value={vec![1_u32, 2]} handle={rows}>|key: u32|
                    <Element tag="b" text={key.to_string()} />
                </ForEach>
            </Element>
        }
    });
    assert_eq!(*rows.keys(), [1, 2]);
    drop(list);
    assert!(rows.keys().is_empty(), "a list gone shows nothing");
    assert_eq!(rows.index_of(&1), None);
    assert!(rows.nodes_of(&1).is_empty());

    let again = Scope::new();
    let _page = again.run(|| {
        view! {
            <Element tag="ul">
                <ForEach value={vec![3_u32]} handle={rows}>|key: u32|
                    <Element tag="b" text={key.to_string()} />
                </ForEach>
            </Element>
        }
    });
    assert_eq!(
        *rows.keys(),
        [3],
        "and the handle may be given to another list after"
    );
}
