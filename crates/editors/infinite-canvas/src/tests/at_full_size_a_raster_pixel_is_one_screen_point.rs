use super::*;
use infinite_canvas_types::DEFAULT_PIXEL_SIZE;

#[test]
fn at_full_size_a_raster_pixel_is_one_screen_point() {
    let mut editor = open(&raster_canvas(DEFAULT_PIXEL_SIZE, &[]), false, &[]);
    editor.click("infinite-canvas.dock.tool.pencil");
    editor.run();
    let center = editor.rect_of("infinite-canvas.canvas").center();

    editor.drag(center + Vec2::new(0.5, 0.5), center + Vec2::new(10.5, 0.5));
    editor.run();
    let layers = rasters(&editor);
    let [layer] = layers.as_slice() else {
        panic!("a stroke on empty canvas makes one layer, made {layers:?}");
    };
    let raster = raster_of(&editor, layer.id);
    assert_eq!(
        layer.transform.size,
        CanvasPoint::new(f32::from(raster.width()), f32::from(raster.height())),
        "at 100% the layer is as many points across as it has pixels"
    );
    assert_eq!(
        painted(&raster),
        11,
        "a stroke ten points long crosses eleven pixels"
    );
}
