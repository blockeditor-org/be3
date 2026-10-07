use super::*;
use crate::Motion;
use crate::reactive::{ForEach, Frame, ItemSize, List, NodeRef, Spacer, build, view};
use crate::unstyled::Scroll;

#[test]
fn a_touch_drag_scrolls_only_once_released_with_gesture_motion_off() {
    let scroll = NodeRef::new();
    let scroll_ref = scroll.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll_ref>
                    <ForEach keys={indices(400)}>
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
    harness.context().set_motion(Motion::Still);
    harness.frame(Vec::new());
    let x = harness.rect(scroll).center().x;

    harness.touch(TouchPhase::Start, pos2(x, 280.0));
    for y in [250.0, 220.0, 190.0, 160.0] {
        harness.touch(TouchPhase::Move, pos2(x, y));
    }

    assert_eq!(
        harness.document().scroll_offset(scroll),
        0.0,
        "the content holds still while the finger is down"
    );

    harness.touch(TouchPhase::End, pos2(x, 160.0));
    let released = harness.document().scroll_offset(scroll);

    assert!(
        released > 100.0 && released <= 120.0,
        "lifting the finger jumps to where the drag ended: {released}"
    );
    assert_eq!(
        harness.settle(),
        0,
        "nothing glides on after the finger lifts"
    );
    assert_eq!(harness.document().scroll_offset(scroll), released);
}
