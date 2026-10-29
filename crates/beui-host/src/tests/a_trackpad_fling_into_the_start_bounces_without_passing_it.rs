use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_trackpad_fling_into_the_start_bounces_without_passing_it() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <ForEach keys={indices(300)}>|index: usize|
                        <LabelledButton label={format!("Row {index}")} on_click={|| {}} />
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let center = harness.rect(scroll).center();
    harness.frame(vec![Input::PointerMoved(center)]);
    harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, -60.0)))]);
    assert_eq!(harness.document().scroll_offset(scroll), 60.0);

    harness.frame(vec![Input::ScrollStarted]);
    for _ in 0..3 {
        harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, 15.0)))]);
    }
    harness.frame(vec![Input::ScrollEnded]);

    let mut deepest = 0.0_f32;
    for _ in 0..240 {
        harness.frame(Vec::new());
        let overscroll = harness.document().scroll_overscroll(scroll);
        assert!(
            overscroll <= 0.0,
            "the bounce never springs past the start towards the end: {overscroll}"
        );
        deepest = deepest.min(overscroll);
    }
    assert!(
        deepest < -1.0,
        "the fling stretches past the start: {deepest}"
    );
    assert_eq!(harness.document().scroll_overscroll(scroll), 0.0);
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);
}
