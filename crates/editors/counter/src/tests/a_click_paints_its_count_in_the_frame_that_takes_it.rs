use super::*;

use block_editor_beui::beui::{Event, Modifiers, PointerButton};

#[test]
fn a_click_paints_its_count_in_the_frame_that_takes_it() {
    let mut harness = Harness::new();
    let pos = harness.editor.rect_of("counter.increment").center();
    let press = |pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };

    harness
        .editor
        .step(vec![Event::PointerMoved(pos), press(true), press(false)]);

    assert_eq!(harness.shown(), "\"1\"");
    harness
        .editor
        .snapshot("a_click_paints_its_count_in_the_frame_that_takes_it");
    harness.run();
    assert_eq!(harness.count(), 1);
    harness
        .editor
        .snapshot("a_click_paints_its_count_in_the_frame_that_takes_it");
}
