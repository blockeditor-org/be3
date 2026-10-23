use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{Scroll, ScrollApi};

#[test]
fn pointing_inside_the_autoscroll_dead_zone_leaves_the_scroll_still() {
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

    harness.frame(vec![Input::PointerMoved(origin)]);
    harness.middle_button(origin, true);
    harness.middle_button(origin, false);
    harness.frame(vec![Input::PointerMoved(origin + Vec2::new(0.0, 5.0))]);
    harness.frame(Vec::new());

    assert!(harness.document().autoscroll.is_some());
    assert!(!harness.document().api::<ScrollApi>(scroll).animating());
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);
}
