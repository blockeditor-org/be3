use super::*;
use block_editor_beui::beui::{PenPhase, PointerButton};

#[test]
fn the_pen_barrel_button_opens_the_context_menu() {
    let rectangle = card();
    let mut editor = editor(std::slice::from_ref(&rectangle));
    editor.click("infinite-canvas.dock.tool.Pen");
    let center = editor
        .rect_of(&format!("infinite-canvas.entity.{}", rectangle.id))
        .center();

    editor.pen(PenPhase::Move, center, 0.0);
    editor.next_frame();
    editor.pen(PenPhase::Down(PointerButton::Secondary), center, 0.0);
    editor.next_frame();
    editor.pen(PenPhase::Up(PointerButton::Secondary), center, 0.0);
    editor.run();
    assert!(
        editor.shown("infinite-canvas.menu.canvas.delete"),
        "the barrel button opens the menu where the pen is"
    );
    assert_eq!(entities(&editor).len(), 1, "and draws nothing");
    editor.click("infinite-canvas.menu.canvas.delete");
    assert!(
        entities(&editor).is_empty(),
        "the menu acts on what the pen pointed at"
    );
}
