use super::*;
use crate::reactive::{ItemSize, List, NodeRef, build, view};
use crate::styled::Scroll;

#[test]
fn scrolling_a_scroll_with_a_scrollbar_lays_it_out_once_a_frame() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let held = scroll;
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&held @sizing=ItemSize::Percent(100.0)>
                    <Frame height=4000.0 />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let over = harness.rect(scroll.get()).center();
    for _ in 0..3 {
        harness.scroll(over, vec2(0.0, -40.0), Modifiers::NONE);
        assert_eq!(
            harness.document().performance().latest.layout_passes,
            1,
            "the scrollbar follows the scroll without a second layout"
        );
    }
    assert!(harness.document().scroll_offset(scroll.get()) > 0.0);
}
