use super::*;
use crate::reactive::{ForEach, ForEachRef};

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "two lists share one ForEachRef")]
fn two_lists_given_one_for_each_handle_is_a_mistake() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let rows = ForEachRef::<u32, DomNode>::new();
    let _page = view! {
        <Element tag="ul">
            <ForEach value={vec![1_u32]} handle={rows}>|key: u32|
                <Element tag="b" text={key.to_string()} />
            </ForEach>
            <ForEach value={vec![2_u32]} handle={rows}>|key: u32|
                <Element tag="b" text={key.to_string()} />
            </ForEach>
        </Element>
    };
}
