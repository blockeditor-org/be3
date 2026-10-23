use super::*;
use block_editor_beui::beui::{Key, Modifiers};

#[test]
fn one_undo_takes_back_a_whole_gesture() {
    let shape = card();
    let mut editor = editor(std::slice::from_ref(&shape));
    let item = format!("infinite-canvas.entity.{}", shape.id);

    let from = editor.rect_of(&item).center();
    editor.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    for step in 1..=12 {
        editor.wait(std::time::Duration::from_millis(400));
        editor.pen(
            PenPhase::Move,
            from + Vec2::new(step as f32 * 10.0, 0.0),
            0.5,
        );
        editor.run();
    }
    editor.pen(
        PenPhase::Up(PointerButton::Primary),
        from + Vec2::new(120.0, 0.0),
        0.0,
    );
    editor.run();
    assert_eq!(
        entities(&editor)[0].transform.center,
        CanvasPoint::new(120.0, 0.0)
    );

    editor.click("infinite-canvas.dock.tool.Pen");
    editor.run();
    let start = editor.rect_of("infinite-canvas.canvas").center() + Vec2::new(0.0, 150.0);
    editor.pen(PenPhase::Down(PointerButton::Primary), start, 0.5);
    for step in 1..=20 {
        editor.next_frame();
        editor.pen(
            PenPhase::Move,
            start + Vec2::new(step as f32 * 8.0, 0.0),
            0.5,
        );
        editor.run();
    }
    editor.pen(
        PenPhase::Up(PointerButton::Primary),
        start + Vec2::new(160.0, 0.0),
        0.0,
    );
    editor.run();
    assert_eq!(entities(&editor).len(), 2);

    editor.click("infinite-canvas.dock.tool.Rectangle");
    editor.run();
    editor.pen(PenPhase::Down(PointerButton::Primary), start, 0.5);
    editor.next_frame();
    editor.pen(PenPhase::Move, start + Vec2::new(80.0, 60.0), 0.5);
    editor.run();
    editor.key(Key::Escape, Modifiers::NONE);
    editor.run();
    editor.pen(
        PenPhase::Up(PointerButton::Primary),
        start + Vec2::new(80.0, 60.0),
        0.0,
    );
    editor.run();
    assert_eq!(
        entities(&editor).len(),
        2,
        "a shape called off while drawn is taken away again"
    );

    undo(&mut editor);
    let undone = entities(&editor);
    assert_eq!(
        undone.len(),
        1,
        "one undo takes back the whole stroke, and the rectangle called off left nothing"
    );
    undo(&mut editor);
    assert_eq!(
        entities(&editor),
        vec![shape],
        "one undo takes back a move however long it took"
    );
}
