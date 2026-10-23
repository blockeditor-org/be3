use super::*;
use crate::mouse_simulation::{SCROLL_TICK, WHEEL_LINE};
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn swiping_the_simulated_middle_button_scrolls_in_ticks() {
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
