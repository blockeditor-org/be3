use super::*;

#[test]
fn a_raster_layer_resized_by_its_handles_keeps_its_pixels() {
    let layer = raster_layer(1, CanvasPoint::default(), 8);
    let mut editor = raster_editor(std::slice::from_ref(&layer));
    paint(&mut editor, layer.id, &[(1, 1)], RED);
    let center = editor.rect_of("infinite-canvas.canvas").center();
    editor.click(center + Vec2::new(4.0, 4.0));
    editor.run();

    let handle = center + Vec2::new(32.0, 0.0);
    editor.drag_along(&[
        handle,
        handle + Vec2::new(10.0, 0.0),
        handle + Vec2::new(20.0, 0.0),
        handle + Vec2::new(33.0, 0.0),
    ]);
    editor.run();
    let raster = raster_of(&editor, layer.id);
    assert_eq!(
        (raster.width(), raster.height()),
        (12, 8),
        "dragging the right handle four pixels out adds four columns"
    );
    let resized = rasters(&editor).remove(0);
    let bounds = entity_bounds(&resized);
    assert_eq!(
        bounds.min,
        CanvasPoint::new(-32.0, -32.0),
        "the left edge stays put"
    );
    assert_eq!(bounds.max, CanvasPoint::new(64.0, 32.0));
    assert_eq!(raster.pixel(1, 1), Some(RED), "and every pixel with it");

    undo(&mut editor);
    assert_eq!(raster_of(&editor, layer.id).width(), 8);

    editor.click(center + Vec2::new(4.0, 4.0));
    editor.run();
    editor.drag(center + Vec2::new(4.0, 4.0), center + Vec2::new(23.0, 4.0));
    editor.run();
    let moved = entity_bounds(&rasters(&editor)[0]);
    assert_eq!(
        moved.min,
        CanvasPoint::new(-16.0, -32.0),
        "a moved layer snaps to the pixel grid"
    );
}
