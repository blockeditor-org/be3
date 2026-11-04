use super::*;

#[test]
fn undoing_a_clear_keeps_a_pixel_painted_since() {
    let (blank, id) = raster_layer(&CanvasContent::default(), 32, 32);
    let mut content = edited(&blank, [paint(&blank, id, 0, 0, RED)]);
    let second = paint(&content, id, 1, 0, RED);
    content.apply(&second);

    let clear = Canvas::edit_in(&content, &InfiniteCanvasOperation::Clear { id });
    let step = content.step(&clear).expect("a clear is undoable");
    content.apply(&clear);
    let repaint = paint(&content, id, 1, 0, BLUE);
    content.apply(&repaint);
    for operation in content.revert(&step) {
        content.apply(&operation);
    }

    let raster = Canvas::raster(&content, id).expect("a raster layer");
    assert_eq!(raster.pixel(0, 0), Some(RED));
    assert_eq!(raster.pixel(1, 0), Some(BLUE));
    assert_eq!(raster.pixel(2, 0), Some(CanvasPixel::TRANSPARENT));
}
