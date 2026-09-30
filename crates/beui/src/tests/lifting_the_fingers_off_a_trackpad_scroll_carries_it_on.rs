use super::*;
use crate::reactive::{ForEach, Frame, ItemSize, List, NodeRef, Spacer, build, view};
use crate::unstyled::Scroll;

#[test]
fn lifting_the_fingers_off_a_trackpad_scroll_carries_it_on() {
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
    harness.frame(Vec::new());
    let pos = harness.rect(scroll).center();

    for _ in 0..6 {
        harness.scroll(pos, vec2(0.0, -20.0), Modifiers::NONE);
    }
    assert_eq!(harness.document().scroll_offset(scroll), 120.0);
    assert!(
        !unstyled::scroll_animating(harness.document(), scroll),
        "a scroll follows the fingers while they are down"
    );

    harness.frame(vec![Event::ScrollEnded]);
    assert!(unstyled::scroll_animating(harness.document(), scroll));
    harness.settle();
    let carried = harness.document().scroll_offset(scroll) - 120.0;
    assert!(
        carried > 100.0 && carried < 800.0,
        "the scroll glides on at the speed the fingers left it: {carried}"
    );

    let rested = harness.document().scroll_offset(scroll);
    harness.scroll(pos, vec2(0.0, -40.0), Modifiers::NONE);
    harness.advance(Duration::from_millis(300));
    harness.frame(vec![Event::ScrollEnded]);
    assert_eq!(
        harness.document().scroll_offset(scroll),
        rested + 40.0,
        "fingers that stopped before they lifted leave the scroll where it is"
    );
    assert!(!unstyled::scroll_animating(harness.document(), scroll));
}
