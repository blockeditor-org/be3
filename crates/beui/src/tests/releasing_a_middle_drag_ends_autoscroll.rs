use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{Scroll, ScrollApi};

#[test]
fn releasing_a_middle_drag_ends_autoscroll() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
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
    let origin = harness.rect(scroll).center();
    let below = origin + Vec2::new(0.0, 100.0);

    harness.frame(vec![Input::PointerMoved(origin)]);
    harness.middle_button(origin, true);
    harness.frame(vec![Input::PointerMoved(below)]);
    harness.frame(Vec::new());
    assert!(harness.document().api::<ScrollApi>(scroll).animating());

    harness.middle_button(below, false);
    harness.frame(Vec::new());

    assert!(harness.document().pointer_grabber().is_none());
    assert!(!harness.document().api::<ScrollApi>(scroll).animating());
    let offset = harness.document().scroll_offset(scroll);
    assert!(offset > 0.0, "the scroll stayed at {offset}");
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(scroll), offset);
}
