use super::*;

#[test]
fn a_raster_layer_added_with_its_pixels_keeps_them_and_its_scale() {
    let (painted, id) = raster_layer(&CanvasContent::default(), 4, 3);
    let painted = edited(&painted, [paint(&painted, id, 2, 1, RED)]);
    let pixels = Canvas::raster(&painted, id).expect("a raster layer");

    let copy = CanvasEntity::new(
        CanvasShape::Raster,
        CanvasTransform::new(
            CanvasPoint::new(10.25, -3.0),
            CanvasPoint::new(12.0, 9.0),
            0.5,
        ),
    );
    let copy_id = copy.id;
    let content = canvas_run(
        &CanvasContent::default(),
        InfiniteCanvasOperation::Add {
            entity: Box::new(copy),
            pixels: Some(pixels.clone()),
        },
    );
    let held = Canvas::raster(&content, copy_id).expect("a raster layer");
    assert_eq!(held, pixels, "the pixels come along");
    assert_eq!(held.pixel(2, 1), Some(RED));
    assert_eq!(
        content.root().entities()[0].transform,
        CanvasTransform::new(
            CanvasPoint::new(10.25, -3.0),
            CanvasPoint::new(12.0, 9.0),
            0.0
        ),
        "and the layer keeps its scale, but never turns"
    );

    assert_eq!(
        CanvasRaster::from_rgba(4, 3, pixels.rgba_bytes()),
        Some(pixels.clone())
    );
    assert_eq!(CanvasRaster::from_rgba(4, 2, pixels.rgba_bytes()), None);
}
