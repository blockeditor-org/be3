use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn fingers_resting_on_the_trackpad_stop_a_fling() {
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
    let animating = |harness: &Harness| {
        harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating()
    };
    let offset = |harness: &Harness| harness.document().scroll_offset(scroll);

    harness.frame(vec![Input::ScrollStarted]);
    for _ in 0..5 {
        harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, -40.0)))]);
    }
    harness.frame(vec![Input::ScrollEnded]);
    harness.frame(Vec::new());
    assert!(animating(&harness), "lifting the fingers flings the list");

    harness.frame(vec![Input::ScrollStarted]);
    let caught = offset(&harness);
    for _ in 0..30 {
        harness.frame(Vec::new());
    }
    assert_eq!(
        offset(&harness),
        caught,
        "fingers resting on the trackpad hold the list where they caught it"
    );

    harness.frame(vec![Input::ScrollEnded]);
    for _ in 0..30 {
        harness.frame(Vec::new());
    }
    assert_eq!(offset(&harness), caught, "lifting them leaves it there");
    assert!(!animating(&harness));
}
