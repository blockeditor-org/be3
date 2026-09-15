use super::*;
use crate::reactive::{ItemSize, NodeRef, Scroll, build, intrinsic, view};

#[test]
fn a_two_finger_drag_on_the_simulated_trackpad_scrolls_smoothly() {
    let scroll = NodeRef::new();
    let scroll_ref = scroll.clone();
    let document = build(move || {
        let items = (0..100)
            .map(|index| {
                intrinsic(view! {
                    <LabelledButton label={format!("Row {index}")} />
                })
            })
            .collect::<Vec<_>>();
        view! {
            <Column spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll_ref children={items} />
            </Column>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.enable_mouse_simulation();
    let center = harness.rect(scroll).center();
    harness.point_at(center);

    let start = harness.simulated_trackpad();
    let apart = Vec2::new(30.0, 0.0);
    let travel = Vec2::new(0.0, -60.0);
    harness.finger(1, TouchPhase::Start, start);
    harness.finger(2, TouchPhase::Start, start + apart);
    harness.finger(1, TouchPhase::Move, start + travel);
    harness.finger(2, TouchPhase::Move, start + apart + travel);
    harness.finger(1, TouchPhase::End, start + travel);
    harness.finger(2, TouchPhase::End, start + apart + travel);

    let offset = harness.document().scroll_offset(scroll);
    assert!(
        (offset - 60.0).abs() < 0.01,
        "the scroll moved to {offset} instead of 60"
    );
    assert!(harness.simulated_cursor().distance(center) < 1.0);
}
