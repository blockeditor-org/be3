use super::*;
use block_editor_beui::beui::{Key, Modifiers};

#[test]
fn a_raster_stroke_is_one_undo_step_and_escape_calls_it_off() {
    let layer = raster_layer(1, CanvasPoint::default(), 16);
    let mut editor = raster_editor(std::slice::from_ref(&layer));
    editor.click("infinite-canvas.dock.tool.pencil");
    editor.run();
    let center = editor.rect_of("infinite-canvas.canvas").center();

    let start = center + Vec2::new(-28.0, 4.0);
    editor.pen(PenPhase::Down(PointerButton::Primary), start, 0.5);
    for step in 1..=6 {
        editor.next_frame();
        editor.pen(
            PenPhase::Move,
            start + Vec2::new(step as f32 * 8.0, 0.0),
            0.5,
        );
        editor.run();
        assert_eq!(
            painted(&raster_of(&editor, layer.id)),
            step + 1,
            "the stroke is written as it is drawn"
        );
    }
    editor.pen(
        PenPhase::Up(PointerButton::Primary),
        start + Vec2::new(48.0, 0.0),
        0.0,
    );
    editor.run();
    assert_eq!(painted(&raster_of(&editor, layer.id)), 7);
    undo(&mut editor);
    assert_eq!(
        painted(&raster_of(&editor, layer.id)),
        0,
        "one undo takes back the whole stroke"
    );

    editor.pen(PenPhase::Down(PointerButton::Primary), start, 0.5);
    editor.next_frame();
    editor.pen(PenPhase::Move, start + Vec2::new(24.0, 0.0), 0.5);
    editor.run();
    assert_eq!(painted(&raster_of(&editor, layer.id)), 4);
    editor.key(Key::Escape, Modifiers::NONE);
    editor.run();
    assert_eq!(
        painted(&raster_of(&editor, layer.id)),
        0,
        "Escape calls the stroke off"
    );
    editor.pen(
        PenPhase::Up(PointerButton::Primary),
        start + Vec2::new(24.0, 0.0),
        0.0,
    );
    editor.run();
    undo(&mut editor);
    assert_eq!(rasters(&editor).len(), 1, "and leaves nothing to undo");
}
