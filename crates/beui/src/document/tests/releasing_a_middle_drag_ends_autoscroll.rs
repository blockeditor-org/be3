use super::*;
use crate::reactive::{ForEach, ItemSize, List, NodeRef, build, view};
use crate::unstyled::{Scroll, scroll_animating};

#[test]
fn releasing_a_middle_drag_ends_autoscroll() {
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
    let below = origin + Vec2::new(0.0, 100.0);

    harness.frame(vec![Event::PointerMoved(origin)]);
    harness.middle_button(origin, true);
    harness.frame(vec![Event::PointerMoved(below)]);
    harness.frame(Vec::new());
    assert!(scroll_animating(harness.document(), scroll));

    harness.middle_button(below, false);
    harness.frame(Vec::new());

    assert!(harness.document().autoscroll.is_none());
    assert!(!scroll_animating(harness.document(), scroll));
    let offset = harness.document().scroll_offset(scroll);
    assert!(offset > 0.0, "the scroll stayed at {offset}");
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(scroll), offset);
}
