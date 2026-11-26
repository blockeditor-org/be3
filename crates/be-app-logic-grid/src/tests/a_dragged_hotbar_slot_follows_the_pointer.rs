use super::*;
use be_sdk_beui::beui::{Input, PointerButton};

#[test]
fn a_dragged_hotbar_slot_follows_the_pointer() {
    let mut editor = editor();
    let slot = editor.rect_of("logic-grid.slot.0");
    let from = slot.center();
    let to = editor.rect_of("logic-grid.canvas").center();

    editor.step(vec![Input::PointerMoved(from)]);
    editor.step(vec![Input::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
    }]);
    editor.step(vec![Input::PointerMoved(to)]);
    editor.run();
    let ghost = editor.rect_of("logic-grid.slot-ghost");
    let held = to - (from - slot.min);
    assert!(
        (ghost.min - held).length() <= 1.0,
        "the dragged slot is drawn under the pointer, held where it was pressed: at {ghost:?}, \
         not {held:?}"
    );
    assert!(editor.label("logic-grid.slot-ghost").contains("Wire"));

    editor.step(vec![Input::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
    }]);
    editor.run();
    assert!(
        !editor.shown("logic-grid.slot-ghost"),
        "the ghost goes with the drop"
    );
    assert!(
        editor.label("logic-grid.slot.0").contains("Wire"),
        "dropping over the canvas leaves the hotbar as it was"
    );
}
