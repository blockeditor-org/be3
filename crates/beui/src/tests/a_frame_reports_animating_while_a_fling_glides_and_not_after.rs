use super::*;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_frame_reports_animating_while_a_fling_glides_and_not_after() {
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
    assert!(!harness.frame(Vec::new()).animating, "nothing moves yet");
    let x = harness.rect(scroll).center().x;

    harness.touch(TouchPhase::Start, pos2(x, 280.0));
    for y in [250.0, 220.0, 190.0, 160.0] {
        harness.touch(TouchPhase::Move, pos2(x, y));
    }
    harness.touch(TouchPhase::End, pos2(x, 160.0));

    let mut frames = 0;
    while harness
        .document()
        .api::<unstyled::ScrollApi>(scroll)
        .animating()
    {
        assert!(frames < 600, "the fling never stopped");
        let output = harness.frame(Vec::new());
        assert!(
            output.animating
                || !harness
                    .document()
                    .api::<unstyled::ScrollApi>(scroll)
                    .animating(),
            "each frame of the fling reports animating"
        );
        frames += 1;
    }
    assert!(frames > 1, "the fling took several frames");
    assert!(
        !harness.frame(Vec::new()).animating,
        "a frame after the fling has stopped reports nothing animating"
    );
}
