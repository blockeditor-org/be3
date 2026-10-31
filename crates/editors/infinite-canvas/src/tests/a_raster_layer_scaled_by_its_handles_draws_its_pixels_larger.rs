use super::*;

#[test]
fn a_raster_layer_scaled_by_its_handles_draws_its_pixels_larger() {
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
        (8, 8),
        "the handles scale the layer, not its pixel count"
    );
    let scaled = rasters(&editor).remove(0);
    let bounds = entity_bounds(&scaled);
    assert_eq!(bounds.min.x, -4.0, "the left edge stays put");
    assert_eq!(
        bounds.size(),
        CanvasPoint::new(12.0, 12.0),
        "it keeps its proportions, snapped to whole units"
    );
    assert_eq!(bounds.min.y, bounds.min.y.round());
    assert_eq!(raster.pixel(1, 1), Some(RED), "and every pixel");

    editor.click("infinite-canvas.dock.tool.pencil");
    editor.run();
    let pixel = 12.0 / 8.0;
    let at = CanvasPoint::new(bounds.min.x + pixel * 5.5, bounds.min.y + pixel * 5.5);
    editor.click(center + Vec2::new(at.x * RASTER_ZOOM, at.y * RASTER_ZOOM));
    editor.run();
    let raster = raster_of(&editor, layer.id);
    assert_eq!(
        raster.pixel(5, 5),
        Some(BLACK),
        "the pencil paints the scaled pixel under it"
    );
    assert_eq!(painted(&raster), 2);
    editor.snapshot("a_raster_layer_scaled_by_its_handles_draws_its_pixels_larger");

    apply(
        &mut editor,
        InfiniteCanvasOperation::Resize {
            id: layer.id,
            width: 4,
            height: 8,
            anchor: infinite_canvas_types::CanvasRasterAnchor::TopLeft,
        },
    );
    editor.run();
    let resized = entity_bounds(&rasters(&editor)[0]);
    assert_eq!(
        resized.size(),
        CanvasPoint::new(6.0, 12.0),
        "resizing the pixels keeps the layer's scale"
    );
    assert_eq!(resized.min, bounds.min, "and its anchored corner");
}
