use super::*;
use crate::reactive::{ForEach, ItemSize, List, NodeRef, build, view};
use crate::unstyled::Scroll;

#[test]
fn a_tap_during_a_fling_stops_it_without_clicking_a_row() {
    let clicks = Rc::new(Cell::new(0));
    let click_sink = clicks.clone();
    let scroll = NodeRef::new();
    let scroll_ref = scroll.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll_ref>
                    <ForEach keys={indices(300)}>
                        {move |index: usize| {
                            let click_sink = click_sink.clone();
                            view! {
                                <LabelledButton
                                    label={format!("Row {index}")}
                                    on_click={move || click_sink.set(click_sink.get() + 1)}
                                />
                            }
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
    harness.finger_drag(&[pos2(x, 280.0), pos2(x, 240.0), pos2(x, 200.0), pos2(x, 160.0)]);
    harness.frame(Vec::new());
    assert!(unstyled::scroll_animating(harness.document(), scroll));

    let tap = pos2(x, 150.0);
    harness.touch(TouchPhase::Start, tap);
    let stopped = harness.document().scroll_offset(scroll);
    harness.touch(TouchPhase::End, tap);
    harness.frame(Vec::new());

    assert!(!unstyled::scroll_animating(harness.document(), scroll));
    assert_eq!(harness.document().scroll_offset(scroll), stopped);
    assert_eq!(clicks.get(), 0, "the tap that stops a fling is not a click");

    let row = pos2(x, harness.rect(scroll).top() + 10.0);
    harness.advance(Duration::from_secs(1));
    harness.touch(TouchPhase::Start, row);
    harness.touch(TouchPhase::End, row);
    assert_eq!(clicks.get(), 1, "once the list is still, a tap clicks the row");
}
