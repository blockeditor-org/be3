use super::*;
use block_editor_beui::beui::{PenPhase, PointerButton};

#[test]
fn the_eraser_end_of_a_pen_deletes_what_it_touches() {
    let first = card();
    let mut second = entity(Uuid::from_u128(2));
    second.transform = CanvasTransform::new(
        CanvasPoint::new(0.0, 200.0),
        CanvasPoint::new(80.0, 80.0),
        0.0,
    );
    let mut locked = entity(Uuid::from_u128(3));
    locked.transform = CanvasTransform::new(
        CanvasPoint::new(0.0, -200.0),
        CanvasPoint::new(80.0, 80.0),
        0.0,
    );
    locked.locked = true;
    let mut editor = editor(&[first.clone(), second.clone(), locked.clone()]);
    editor.click("infinite-canvas.dock.tool.Pen");
    let at = |editor: &BeuiTest<CanvasApp>, entity: &CanvasEntity| {
        editor
            .rect_of(&format!("infinite-canvas.entity.{}", entity.id))
            .center()
    };
    let (top, middle) = (at(&editor, &locked), at(&editor, &first));

    editor.eraser(PenPhase::Move, top);
    editor.next_frame();
    editor.eraser(PenPhase::Down(PointerButton::Primary), top);
    for step in 1..=8 {
        editor.next_frame();
        editor.eraser(PenPhase::Move, top + (middle - top) * (step as f32 / 8.0));
    }
    editor.next_frame();
    editor.eraser(PenPhase::Up(PointerButton::Primary), middle);
    editor.run();

    assert_eq!(
        entities(&editor),
        vec![second.clone(), locked.clone()],
        "the eraser deletes what it crosses, leaves what is locked and draws nothing"
    );
    undo(&mut editor);
    assert_eq!(
        entities(&editor),
        vec![first, second, locked],
        "one undo brings back everything one stroke erased"
    );
}
