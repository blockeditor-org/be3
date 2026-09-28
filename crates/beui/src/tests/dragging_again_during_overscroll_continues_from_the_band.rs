use super::*;
use crate::reactive::{ForEach, Frame, ItemSize, List, NodeRef, Spacer, build, view};
use crate::unstyled::Scroll;

#[test]
fn dragging_again_during_overscroll_continues_from_the_band() {
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

    harness.touch(TouchPhase::Start, pos2(x, 20.0));
    harness.touch(TouchPhase::Move, pos2(x, 220.0));
    harness.touch(TouchPhase::End, pos2(x, 220.0));
    let released = harness.document().scroll_overscroll(scroll);
    assert!(released < 0.0);

    harness.touch(TouchPhase::Start, pos2(x, 100.0));
    let grabbed = harness.document().scroll_overscroll(scroll);
    assert!(grabbed < 0.0);
    harness.touch(TouchPhase::Move, pos2(x, 120.0));
    let dragged = harness.document().scroll_overscroll(scroll);

    assert!(dragged < grabbed);
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);
}
