use super::*;
use crate::Motion;
use crate::reactive::{ForEach, Frame, ItemSize, List, NodeRef, Spacer, build, view};
use crate::unstyled::Scroll;

#[test]
fn turning_off_animations_in_the_inspector_stops_a_scroll_at_its_end() {
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
    assert_eq!(harness.document().motion(), Motion::Animated);

    harness.toggle_inspector();
    harness.click(harness.simulation_tab_center());
    harness.frame(Vec::new());
    harness.click(harness.animations_toggle_center());
    harness.frame(Vec::new());
    harness.toggle_inspector();
    harness.frame(Vec::new());
    assert_eq!(harness.document().motion(), Motion::Instant);

    let x = harness.rect(scroll).center().x;
    let top = harness.rect(scroll).top();
    harness.touch(TouchPhase::Start, pos2(x, top + 20.0));
    harness.touch(TouchPhase::Move, pos2(x, top + 220.0));

    assert_eq!(harness.document().scroll_overscroll(scroll), 0.0);
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);

    harness.touch(TouchPhase::Move, pos2(x, top + 180.0));

    assert_eq!(
        harness.document().scroll_offset(scroll),
        40.0,
        "dragging back scrolls at once instead of first undoing the drag past the end"
    );
}
