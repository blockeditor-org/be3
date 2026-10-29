use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::Scroll;

const DRAGGED: f32 = 100.0;

#[test]
fn a_finger_on_a_hidden_scrollbar_scrolls_the_content_under_it() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame height=2000.0 />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let bar = harness.rect(scrollbar_of(&harness, scroll.get()));
    let from = pos2(bar.center().x, bar.center().y);

    harness.touch(TouchPhase::Start, from);
    for step in 1..=10 {
        harness.touch(
            TouchPhase::Move,
            pos2(from.x, from.y - DRAGGED * step as f32 / 10.0),
        );
    }
    let offset = harness.document().scroll_offset(scroll.get());
    assert!(
        offset > DRAGGED / 2.0 && offset <= DRAGGED,
        "the finger dragged the content {DRAGGED} rather than the thumb: {offset}"
    );
    harness.touch(TouchPhase::End, pos2(from.x, from.y - DRAGGED));
}
