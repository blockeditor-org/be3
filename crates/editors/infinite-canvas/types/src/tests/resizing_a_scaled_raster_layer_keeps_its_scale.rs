use super::*;
use crate::CanvasRasterAnchor;

#[test]
fn resizing_a_scaled_raster_layer_keeps_its_scale() {
    let (content, id) = raster_layer(&CanvasContent::default(), 4, 2);
    let mut scaled = content.root().entities()[0].clone();
    scaled.transform.size = CanvasPoint::new(40.0, 10.0);
    let content = canvas_run(
        &content,
        InfiniteCanvasOperation::Update {
            entities: vec![scaled],
        },
    );
    let content = edited(&content, [paint(&content, id, 0, 0, RED)]);
    let resized = canvas_run(
        &content,
        InfiniteCanvasOperation::Resize {
            id,
            width: 6,
            height: 3,
            anchor: CanvasRasterAnchor::TopLeft,
        },
    );
    let held = resized.root().entities()[0].clone();
    assert_eq!(
        held.transform.size,
        CanvasPoint::new(60.0, 15.0),
        "ten by five units a pixel, as before"
    );
    assert_eq!(
        held.transform.to_world(CanvasPoint::new(-0.5, -0.5)),
        CanvasPoint::new(-20.0, -5.0),
        "the anchored corner stays put"
    );
    let raster = Canvas::raster(&resized, id).expect("a raster layer");
    assert_eq!(raster.pixel(0, 0), Some(RED));
}
