use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn an_overflowing_scroll_of_plain_text_is_a_tab_stop() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <Scroll @node_ref=&scroll>
                <ForEach keys={indices(20)}>|_: usize|
                    <Text color=Color32::WHITE>"Content"</Text>
                </ForEach>
            </Scroll>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::sized(document, Vec2::new(300.0, 100.0));
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    let focused = harness
        .document()
        .focused_node()
        .expect("the scroll takes the focus");
    assert!(
        harness
            .document()
            .focusables_within(scroll)
            .contains(&focused)
    );
    harness.key(Key::PageDown, Modifiers::NONE);
    assert_eq!(harness.document().scroll_offset(scroll), 100.0);
}
