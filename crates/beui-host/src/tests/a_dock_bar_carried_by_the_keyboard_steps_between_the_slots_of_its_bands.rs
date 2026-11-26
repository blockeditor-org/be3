use super::*;
use crate::dock::{DockAlign, DockBarPlacement, DockEdge};

#[test]
fn a_dock_bar_carried_by_the_keyboard_steps_between_the_slots_of_its_bands() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = bar_dock(
        DockBarPlacement::Beside(DockEdge::Top, DockAlign::Start),
        false,
    );
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let grip = harness.find("dock.bar.9.grip");
    let button = harness.document().focusables_within(grip)[0];
    harness.document_mut().focus_focusable(button);
    harness.frame(Vec::new());

    let ctrl_shift = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
    harness.key(Key::ArrowRight, ctrl_shift);
    harness.settle();
    assert_eq!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Beside(DockEdge::Top, DockAlign::Center)),
        "Ctrl+Shift and Right move the focused grip's bar to the next slot along its band"
    );
    harness.key(Key::ArrowDown, ctrl_shift);
    harness.settle();
    assert!(
        matches!(
            dock.layout.bar_placement(&9),
            Some(DockBarPlacement::Beside(
                DockEdge::Left | DockEdge::Right | DockEdge::Bottom,
                _
            ))
        ),
        "and Down into a slot of a band below it: {:?}",
        dock.layout.bar_placement(&9)
    );
}
