use super::*;
use crate::reactive::{ForEach, Frame, ItemSize, List, NodeRef, Spacer, build, view};
use crate::unstyled::Scroll;

#[test]
fn a_touch_fling_that_ends_without_moving_keeps_its_momentum() {
    let scroll = NodeRef::new();
    let scroll_ref = scroll.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll_ref>
                    <ForEach keys={indices(40)}>
                        {|_: usize| view! {
                            <Frame padding_horizontal=0.0 padding_vertical=20.0>
                                <Spacer />
                            </Frame>
                        }}
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let x = harness.rect(scroll).center().x;
    let start = pos2(x, 300.0);
    let end = pos2(x, 200.0);

    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::Move, pos2(x, 250.0));
    harness.touch(TouchPhase::Move, end);
    let released = harness.frame(vec![touch_event(1, TouchPhase::End, end)]);

    assert!(unstyled::scroll_animating(harness.document(), scroll));
    assert!(released.repaint || released.repaint_after == Duration::ZERO);
}
