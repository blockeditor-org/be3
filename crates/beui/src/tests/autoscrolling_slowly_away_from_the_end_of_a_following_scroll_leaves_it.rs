use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::Scroll;
use beui_core::interact::autoscroll::AUTOSCROLL_DEAD_ZONE;

#[test]
fn autoscrolling_slowly_away_from_the_end_of_a_following_scroll_leaves_it() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll follow_end=true>
                    <ForEach keys={indices(100)}>|index: usize|
                        <LabelledButton label={format!("Row {index}")} />
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let end = harness.document().scroll_offset(scroll);
    assert!(end > 0.0);
    let origin = harness.rect(scroll).center();

    harness.frame(vec![Input::PointerMoved(origin)]);
    harness.middle_button(origin, true);
    harness.middle_button(origin, false);
    harness.frame(vec![Input::PointerMoved(
        origin - Vec2::new(0.0, AUTOSCROLL_DEAD_ZONE + 2.0),
    )]);
    for _ in 0..30 {
        harness.frame(Vec::new());
    }

    let offset = harness.document().scroll_offset(scroll);
    assert!(offset < end - 1.0, "the scroll stayed at {offset} of {end}");
}
