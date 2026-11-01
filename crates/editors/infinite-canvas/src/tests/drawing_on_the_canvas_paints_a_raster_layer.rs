use super::*;

#[test]
fn drawing_on_the_canvas_paints_a_raster_layer() {
    let mut editor = raster_editor(&[]);
    editor.click("infinite-canvas.dock.tool.pencil");
    editor.run();
    let center = editor.rect_of("infinite-canvas.canvas").center();

    editor.drag(center + Vec2::new(4.0, 4.0), center + Vec2::new(44.0, 4.0));
    editor.run();
    let layers = rasters(&editor);
    let [layer] = layers.as_slice() else {
        panic!("a stroke on empty canvas makes one layer, made {layers:?}");
    };
    let raster = raster_of(&editor, layer.id);
    assert_eq!((raster.width(), raster.height()), (32, 32));
    assert!(
        entity_bounds(layer).contains(CanvasPoint::new(0.5, 0.5)),
        "the layer is centred on where the stroke began"
    );
    let black: Vec<(u16, u16)> = (0..32)
        .flat_map(|y| (0..32).map(move |x| (x, y)))
        .filter(|&(x, y)| raster.pixel(x, y) == Some(BLACK))
        .collect();
    assert_eq!(
        black.len(),
        6,
        "one pixel for each pixel the stroke crossed, painted {black:?} in {layer:?}"
    );
    assert_eq!(raster.pixel(16, 16), Some(BLACK));
    assert_eq!(raster.pixel(21, 16), Some(BLACK));

    editor.drag(center + Vec2::new(4.0, 20.0), center + Vec2::new(4.0, 60.0));
    editor.run();
    assert_eq!(
        rasters(&editor).len(),
        1,
        "a stroke on a layer paints into it"
    );
    assert_eq!(painted(&raster_of(&editor, layer.id)), 12);

    editor.drag(
        center + Vec2::new(-200.0, 0.0),
        center + Vec2::new(-180.0, 0.0),
    );
    editor.run();
    assert_eq!(painted(&raster_of(&editor, layer.id)), 12);
    assert_eq!(rasters(&editor).len(), 2);
    editor.snapshot("drawing_on_the_canvas_paints_a_raster_layer");
}
