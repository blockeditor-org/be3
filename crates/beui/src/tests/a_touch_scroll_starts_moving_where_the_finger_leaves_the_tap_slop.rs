use super::*;
use crate::input::TOUCH_DRAG_THRESHOLD;
use crate::reactive::{ForEach, ItemSize, List, NodeRef, build, view};
use crate::unstyled::Scroll;

#[test]
fn a_touch_scroll_starts_moving_where_the_finger_leaves_the_tap_slop() {
    let scroll = NodeRef::new();
    let scroll_ref = scroll.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll_ref>
                    <ForEach keys={indices(100)}>
                        {|index: usize| view! {
                            <LabelledButton label={format!("Row {index}")} on_click={|| {}} />
                        }}
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let rect = harness.rect(scroll);
    let start = pos2(rect.center().x, rect.bottom() - 40.0);

    harness.touch(TouchPhase::Start, start);
    harness.touch(
        TouchPhase::Move,
        pos2(start.x, start.y - TOUCH_DRAG_THRESHOLD),
    );
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);

    harness.touch(
        TouchPhase::Move,
        pos2(start.x, start.y - TOUCH_DRAG_THRESHOLD - 1.0),
    );
    assert!((harness.document().scroll_offset(scroll) - 1.0).abs() < 0.01);

    harness.touch(
        TouchPhase::Move,
        pos2(start.x, start.y - TOUCH_DRAG_THRESHOLD - 5.0),
    );
    assert!((harness.document().scroll_offset(scroll) - 5.0).abs() < 0.01);
}
