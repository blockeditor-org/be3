use super::*;
use crate::reactive::{ForEach, ItemSize, List, NodeRef, build, view};
use crate::unstyled::Scroll;

#[test]
fn a_second_swipe_during_a_fling_keeps_scrolling() {
    let scroll = NodeRef::new();
    let scroll_ref = scroll.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll_ref>
                    <ForEach keys={indices(300)}>
                        {move |index: usize| view! {
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
    let x = harness.rect(scroll).center().x;
    let swipe = [
        pos2(x, 280.0),
        pos2(x, 240.0),
        pos2(x, 200.0),
        pos2(x, 160.0),
    ];
    harness.finger_drag(&swipe);
    harness.frame(Vec::new());
    assert!(unstyled::scroll_animating(harness.document(), scroll));

    harness.touch(TouchPhase::Start, swipe[0]);
    let caught = harness.document().scroll_offset(scroll);
    for point in &swipe[1..] {
        harness.touch(TouchPhase::Move, *point);
    }
    assert!(
        harness.document().scroll_offset(scroll) >= caught + 100.0,
        "the second swipe moves the list with the finger: {} after catching it at {caught}",
        harness.document().scroll_offset(scroll),
    );
    harness.touch(TouchPhase::End, swipe[3]);
    harness.frame(Vec::new());
    assert!(
        unstyled::scroll_animating(harness.document(), scroll),
        "letting go of the second swipe flings again"
    );
}
