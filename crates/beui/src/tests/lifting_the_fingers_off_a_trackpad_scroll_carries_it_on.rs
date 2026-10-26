use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn lifting_the_fingers_off_a_trackpad_scroll_carries_it_on() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <ForEach keys={indices(400)}>|_: usize|
                        <Frame padding={Sides::symmetric(0.0, 20.0)}>
                            <Spacer />
                        </Frame>
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let pos = harness.rect(scroll).center();

    harness.frame(vec![Input::PointerMoved(pos)]);
    for _ in 0..6 {
        harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, -20.0)))]);
    }
    assert_eq!(harness.document().scroll_offset(scroll), 120.0);
    assert!(
        !harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating(),
        "a scroll follows the fingers while they are down"
    );

    harness.frame(vec![Input::ScrollEnded]);
    assert!(
        harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating()
    );
    harness.settle();
    let carried = harness.document().scroll_offset(scroll) - 120.0;
    assert!(
        carried > 100.0 && carried < 800.0,
        "the scroll glides on at the speed the fingers left it: {carried}"
    );

    let rested = harness.document().scroll_offset(scroll);
    harness.scroll_at(pos, vec2(0.0, -40.0), Modifiers::NONE);
    harness.advance(Duration::from_millis(300));
    harness.frame(vec![Input::ScrollEnded]);
    assert_eq!(
        harness.document().scroll_offset(scroll),
        rested + 40.0,
        "fingers that stopped before they lifted leave the scroll where it is"
    );
    assert!(
        !harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating()
    );
}
