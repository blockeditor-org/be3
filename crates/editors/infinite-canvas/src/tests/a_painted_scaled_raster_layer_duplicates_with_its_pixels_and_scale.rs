use super::*;
use block_editor_beui::beui::{Key, Modifiers};

#[test]
fn a_painted_scaled_raster_layer_duplicates_with_its_pixels_and_scale() {
    let (mut editor, layer) = painted_scaled_layer();
    let center = editor.rect_of("infinite-canvas.canvas").center();
    let original = rasters(&editor).remove(0);
    let applied = editor.applied(None);

    editor.key(Key::D, Modifiers::CTRL);
    editor.run();
    assert_eq!(editor.applied(None), applied + 1);
    let [_, copy] = &rasters(&editor)[..] else {
        panic!("Ctrl+D adds one layer");
    };
    assert_eq!(
        copy.transform,
        CanvasTransform::new(
            CanvasPoint::new(24.0, 24.0),
            CanvasPoint::new(16.0, 16.0),
            0.0
        ),
        "the copy keeps the layer's scale"
    );
    assert_eq!(
        raster_of(&editor, copy.id),
        raster_of(&editor, layer),
        "and its pixels"
    );
    editor.snapshot("a_painted_scaled_raster_layer_duplicates_with_its_pixels_and_scale");

    undo(&mut editor);
    assert_eq!(
        rasters(&editor),
        vec![original.clone()],
        "undo takes the copy away"
    );

    editor.click(center + Vec2::new(4.0, 4.0));
    editor.run();
    editor.hold_modifiers(Modifiers::ALT);
    let from = center + Vec2::new(4.0, 4.0);
    editor.drag_along(&[
        from,
        from + Vec2::new(40.0, 0.0),
        from + Vec2::new(160.0, 0.0),
    ]);
    editor.hold_modifiers(Modifiers::NONE);
    editor.run();
    let [held, dragged] = &rasters(&editor)[..] else {
        panic!("an Alt-drag adds one layer");
    };
    assert_eq!(held, &original, "and leaves the original where it was");
    assert_eq!(dragged.transform.size, CanvasPoint::new(16.0, 16.0));
    assert_eq!(dragged.transform.center, CanvasPoint::new(20.0, 0.0));
    assert_eq!(raster_of(&editor, dragged.id), raster_of(&editor, layer));
}
