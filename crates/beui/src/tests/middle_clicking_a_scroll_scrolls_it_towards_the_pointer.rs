use super::*;
use crate::reactive::{ForEach, ItemSize, List, NodeRef, build, view};
use crate::styled::Scroll;

#[test]
fn middle_clicking_a_scroll_scrolls_it_towards_the_pointer() {
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
    harness.frame(Vec::new());

    let markers = harness.document().overlay_rects();
    assert!(
        markers
            .iter()
            .any(|marker| marker.center().distance(origin) < 1.0),
        "no marker is centred on {origin:?}: {markers:?}"
    );
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);

    harness.frame(vec![Event::PointerMoved(origin + Vec2::new(0.0, 100.0))]);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let offset = harness.document().scroll_offset(scroll);
    assert!(offset > 0.0, "the scroll stayed at {offset}");

    harness.click(origin + Vec2::new(0.0, 100.0));
    harness.frame(Vec::new());
    assert_eq!(harness.document().overlay_rects(), Vec::new());
}
