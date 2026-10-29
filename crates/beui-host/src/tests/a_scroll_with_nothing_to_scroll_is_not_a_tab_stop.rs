use super::*;
use crate::primitives::Text;
use crate::unstyled::Scroll;

#[test]
fn a_scroll_with_nothing_to_scroll_is_not_a_tab_stop() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <Scroll>
                <Text color=Color32::WHITE>"Short"</Text>
            </Scroll>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(300.0, 100.0));
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(harness.document().focused_node(), None);
}
