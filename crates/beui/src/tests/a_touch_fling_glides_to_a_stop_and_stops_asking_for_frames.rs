use super::*;
use crate::reactive::{ForEach, Frame, ItemSize, List, NodeRef, Spacer, build, view};
use crate::unstyled::Scroll;

#[test]
fn a_touch_fling_glides_to_a_stop_and_stops_asking_for_frames() {
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
    let x = harness.rect(scroll).center().x;

    harness.touch(TouchPhase::Start, pos2(x, 280.0));
    for y in [250.0, 220.0, 190.0, 160.0] {
        harness.touch(TouchPhase::Move, pos2(x, y));
    }
    harness.touch(TouchPhase::End, pos2(x, 160.0));
    let released = harness.document().scroll_offset(scroll);

    let mut offsets = vec![released];
    while unstyled::scroll_animating(harness.document(), scroll) {
        assert!(offsets.len() < 600, "the fling never stopped");
        harness.frame(Vec::new());
        offsets.push(harness.document().scroll_offset(scroll));
    }
    let rested = *offsets.last().expect("an offset");
    let steps: Vec<f32> = offsets.windows(2).map(|pair| pair[1] - pair[0]).collect();

    assert!(
        rested - released > 200.0,
        "a fling at about 1800 points a second carries the content well past the finger: {released} -> {rested}"
    );
    assert!(
        steps.iter().all(|step| *step >= 0.0),
        "the fling never moves backwards"
    );
    assert!(
        steps.windows(2).all(|pair| pair[1] <= pair[0] + 0.01),
        "the fling only ever slows down: {steps:?}"
    );
    assert_eq!(
        harness.settle(),
        0,
        "once the fling has stopped nothing asks for another frame"
    );
    assert_eq!(harness.document().scroll_offset(scroll), rested);
}
