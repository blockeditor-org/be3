use super::*;
use crate::Motion;
use crate::reactive::{ForEach, ItemSize, List, NodeRef, build, view};
use crate::styled::Scroll;

#[test]
fn autoscroll_moves_once_a_second_with_gesture_motion_off() {
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
    harness.context().set_motion(Motion::Still);
    harness.frame(Vec::new());
    let origin = harness.rect(scroll).center();

    harness.frame(vec![Event::PointerMoved(origin)]);
    harness.middle_button(origin, true);
    harness.middle_button(origin, false);
    let output = harness.frame(vec![Event::PointerMoved(origin + Vec2::new(0.0, 100.0))]);

    assert!(
        output.repaint_after > Duration::from_millis(900)
            && output.repaint_after <= Duration::from_secs(1),
        "the next step is about a second away, not the next frame: {:?}",
        output.repaint_after
    );
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);

    harness.advance(Duration::from_secs(1));
    harness.frame(Vec::new());
    let stepped = harness.document().scroll_offset(scroll);
    assert!(stepped > 0.0, "a second later the scroll moved once");

    let output = harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(scroll), stepped);
    assert!(output.repaint_after > Duration::ZERO);
}
