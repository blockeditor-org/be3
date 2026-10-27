use super::*;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_touch_fling_that_ends_without_moving_keeps_its_momentum() {
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
    let start = pos2(x, 299.0);
    let end = pos2(x, 200.0);

    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::Move, pos2(x, 250.0));
    harness.touch(TouchPhase::Move, end);
    let released = harness.touch(TouchPhase::End, end);

    assert!(
        harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating()
    );
    assert!(released.repaint || released.repaint_after == Duration::ZERO);
}
