use super::*;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn setting_a_scroll_offset_during_a_fling_stops_the_fling() {
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
    let x = harness.rect(scroll).center().x;

    harness.touch(TouchPhase::Start, pos2(x, 280.0));
    for y in [250.0, 220.0, 190.0, 160.0] {
        harness.touch(TouchPhase::Move, pos2(x, y));
    }
    harness.touch(TouchPhase::End, pos2(x, 160.0));
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating()
    );

    harness.document_mut().set_scroll_offset(scroll, 100.0);
    assert!(
        !harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating(),
        "setting the offset stops the fling as it is set"
    );
    for _ in 0..10 {
        harness.frame(Vec::new());
    }
    assert_eq!(harness.document().scroll_offset(scroll), 100.0);
}
