use super::*;
use crate::reactive::{ForEach, ItemSize, List, NodeRef, build, view};
use crate::unstyled::{Scroll, scroll_animating};

#[test]
fn pointing_inside_the_autoscroll_dead_zone_leaves_the_scroll_still() {
    let scroll = NodeRef::new();
    let scroll_ref = scroll.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll_ref>
                    <ForEach keys={indices(100)}>
                        {|index: usize| view! {
                            <LabelledButton label={format!("Row {index}")} />
                        }}
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let origin = harness.rect(scroll).center();

    harness.frame(vec![Event::PointerMoved(origin)]);
    harness.middle_button(origin, true);
    harness.middle_button(origin, false);
    harness.frame(vec![Event::PointerMoved(origin + Vec2::new(0.0, 5.0))]);
    harness.frame(Vec::new());

    assert!(harness.document().autoscroll.is_some());
    assert!(!scroll_animating(harness.document(), scroll));
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);
}
