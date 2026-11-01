use super::*;
use block_editor_beui::beui::{Key, Modifiers};

#[test]
fn fill_and_the_eyedropper_work_on_the_layer_under_the_pointer() {
    let layer = raster_layer(1, CanvasPoint::default(), 4);
    let mut editor = raster_editor(std::slice::from_ref(&layer));
    paint(&mut editor, layer.id, &[(0, 0), (1, 0), (0, 1)], RED);
    let center = editor.rect_of("infinite-canvas.canvas").center();
    let pixel = |x: f32, y: f32| center + Vec2::new(-16.0 + 8.0 * x + 4.0, -16.0 + 8.0 * y + 4.0);

    editor.key(Key::G, Modifiers::NONE);
    editor.run();
    editor.click(pixel(3.0, 3.0));
    editor.run();
    let raster = raster_of(&editor, layer.id);
    assert_eq!(painted(&raster), 16);
    assert_eq!(
        raster.pixel(3, 3),
        Some(BLACK),
        "the fill floods the clear pixels"
    );
    assert_eq!(raster.pixel(0, 0), Some(RED), "and stops at another colour");

    editor.click("infinite-canvas.dock.tool.pencil");
    editor.key(Key::I, Modifiers::NONE);
    editor.run();
    editor.click(pixel(0.0, 0.0));
    editor.run();
    assert!(
        pressed(&editor, "infinite-canvas.dock.tool.pencil"),
        "the eyedropper hands back the tool it was picked from"
    );
    editor.click(pixel(3.0, 3.0));
    editor.run();
    assert_eq!(
        raster_of(&editor, layer.id).pixel(3, 3),
        Some(RED),
        "the pencil paints in the colour picked"
    );

    editor.key(Key::C, Modifiers::NONE);
    editor.run();
    editor.click(pixel(2.0, 2.0));
    editor.run();
    let raster = raster_of(&editor, layer.id);
    assert_eq!(raster.pixel(2, 2), Some(RED));
    assert_eq!(
        raster.pixel(3, 2),
        Some(RED),
        "replacing a colour changes it everywhere"
    );
}
