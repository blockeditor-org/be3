use super::*;
use crate::reactive::{Dynamic, ForEach, ForEachRef, flush};

#[test]
fn a_list_rebuilt_under_its_for_each_handle_takes_it_over() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let rows = ForEachRef::<u32, DomNode>::new();
    let page = create_signal(1_u32);
    let _page = view! {
        <Element tag="ul">
            <Dynamic value={page}>|page: u32|
                <ForEach value={vec![page * 10, page * 10 + 1]} handle={rows}>|key: u32|
                    <Element tag="b" text={key.to_string()} />
                </ForEach>
            </Dynamic>
        </Element>
    };
    assert_eq!(*rows.keys(), [10, 11]);

    page.set(2);
    flush();
    assert_eq!(
        *rows.keys(),
        [20, 21],
        "the list built in place of the old one holds the handle, and the old one's cleanup \
         leaves it alone"
    );
    assert_eq!(rows.nodes_of(&20).len(), 1);
    assert!(rows.nodes_of(&10).is_empty());
}
