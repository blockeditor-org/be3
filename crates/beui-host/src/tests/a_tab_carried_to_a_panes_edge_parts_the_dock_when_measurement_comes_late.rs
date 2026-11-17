use super::*;
use crate::dock_host::unstyled::DockApi;

#[test]
fn a_tab_carried_to_a_panes_edge_parts_the_dock_when_measurement_comes_late() {
    let (prompt, late) = prompt_and_late(|| {
        let _root = crate::reactive::RootScope::new();
        let (document, dock) = dock_of(2);
        let mut harness = Harness::sized(document, WIDE_VIEWPORT);
        harness.settle();
        // The pane shows the tab that stays, so its panel is on screen while the other is carried.
        harness.click(harness.center(dock_tab(harness.document(), dock, "Tab 1")));
        harness.settle();
        let before = harness.rect(harness.find("content.1"));
        let from = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
        let edge = pos2(WIDE_VIEWPORT.x - 20.0, WIDE_VIEWPORT.y / 2.0);
        harness.down(from, PointerButton::Primary);
        for step in 1..=8 {
            harness.hover(from + (edge - from) * (step as f32 / 8.0));
        }
        harness.pause(Duration::from_millis(400));
        let opening = harness
            .document()
            .find_test_id("dock.drop")
            .map(|drop| harness.rect(drop));
        let held = harness.rect(harness.find("content.1")).size();
        harness.up(edge, PointerButton::Primary);
        harness.frames_to_settle();
        let state = harness.document().api::<DockApi>(dock).state();
        let split = state.leaves(state.main()).len();
        let left = harness.rect(harness.find("content.1"));
        let right = harness.rect(harness.find("content.2"));
        (before.size(), opening, held, split, left, right)
    });
    for (before, opening, held, split, left, right) in [prompt, late] {
        let opening = opening.expect("the carried tab is drawn as the opening it would make");
        assert!(
            opening.left() > WIDE_VIEWPORT.x * 0.4,
            "the opening is the right half of the pane: {opening:?}"
        );
        assert_eq!(
            held, before,
            "the panel beside it keeps its size while carried"
        );
        assert_eq!(split, 2, "let go, the pane splits");
        assert!(left.right() <= right.left(), "the new pane is on the right");
    }
    assert_eq!(
        prompt, late,
        "measured a frame late, the dock parts the same"
    );
}
