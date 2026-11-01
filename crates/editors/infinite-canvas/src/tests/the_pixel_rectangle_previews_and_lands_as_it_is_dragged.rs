use super::*;
use block_editor_beui::beui::{Key, Modifiers};

#[test]
fn the_pixel_rectangle_previews_and_lands_as_it_is_dragged() {
    let layer = raster_layer(1, CanvasPoint::default(), 8);
    let mut editor = raster_editor(std::slice::from_ref(&layer));
    editor.key(Key::R, Modifiers::NONE);
    editor.run();
    let center = editor.rect_of("infinite-canvas.canvas").center();
    let pixel = |x: f32, y: f32| center + Vec2::new(-32.0 + 8.0 * x + 4.0, -32.0 + 8.0 * y + 4.0);

    editor.pen(PenPhase::Down(PointerButton::Primary), pixel(1.0, 1.0), 0.5);
    editor.next_frame();
    editor.pen(PenPhase::Move, pixel(6.0, 6.0), 0.5);
    editor.run();
    assert_eq!(
        painted(&raster_of(&editor, layer.id)),
        20,
        "a 6 by 6 outline"
    );
    editor.next_frame();
    editor.pen(PenPhase::Move, pixel(3.0, 3.0), 0.5);
    editor.run();
    assert_eq!(
        painted(&raster_of(&editor, layer.id)),
        8,
        "shrinking it puts back the pixels it no longer covers"
    );
    editor.pen(PenPhase::Up(PointerButton::Primary), pixel(3.0, 3.0), 0.0);
    editor.run();

    editor.key(Key::X, Modifiers::NONE);
    editor.key(Key::H, Modifiers::NONE);
    editor.run();
    editor.drag(pixel(0.0, 5.0), pixel(1.0, 6.0));
    editor.run();
    let raster = raster_of(&editor, layer.id);
    assert_eq!(
        painted(&raster),
        16,
        "a filled square, mirrored left to right"
    );
    assert_eq!(raster.pixel(7, 6), Some(BLACK));
}
