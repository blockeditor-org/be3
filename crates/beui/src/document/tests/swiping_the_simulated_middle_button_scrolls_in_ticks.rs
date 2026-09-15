use super::*;
use crate::mouse_simulation::{SCROLL_TICK, WHEEL_LINE};
use crate::reactive::{ItemSize, NodeRef, Scroll, build, intrinsic, view};

#[test]
fn swiping_the_simulated_middle_button_scrolls_in_ticks() {
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
    harness.point_at(harness.rect(scroll).center());

    let middle = harness.simulated_button(1);
    let travel = Vec2::new(0.0, -(SCROLL_TICK * 3.0 + 1.0));
    harness.finger(1, TouchPhase::Start, middle);
    harness.finger(1, TouchPhase::Move, middle + travel);
    harness.finger(1, TouchPhase::End, middle + travel);

    let offset = harness.document().scroll_offset(scroll);
    let expected = WHEEL_LINE * 3.0;
    assert!(
        (offset - expected).abs() < 0.01,
        "the scroll moved to {offset} instead of {expected}"
    );
}
