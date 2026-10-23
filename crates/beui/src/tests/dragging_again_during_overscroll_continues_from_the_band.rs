use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn dragging_again_during_overscroll_continues_from_the_band() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <ForEach keys={indices(40)}>|_: usize|
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
    assert_eq!(harness.document().scroll_offset(scroll), dragged);
}
