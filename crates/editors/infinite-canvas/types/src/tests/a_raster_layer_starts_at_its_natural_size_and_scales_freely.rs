use super::*;

#[test]
fn a_raster_layer_starts_at_its_natural_size_and_scales_freely() {
    let blank = CanvasContent::default();
    let layer = CanvasEntity::new(
        CanvasShape::Raster,
        CanvasTransform::new(
            CanvasPoint::new(13.3, -5.0),
            CanvasPoint::new(3.4, 2.0),
            0.7,
        ),
    );
    let id = layer.id;
    let content = canvas_run(
        &blank,
        InfiniteCanvasOperation::Add {
            entity: Box::new(layer),
            pixels: None,
        },
    );
    let raster = Canvas::raster(&content, id).expect("a raster layer");
    assert_eq!((raster.width(), raster.height()), (3, 2));
    let entity = content.root().entities()[0].clone();
    assert_eq!(entity.transform.rotation, 0.0);
    assert_eq!(
        entity.transform.size,
        CanvasPoint::new(3.0, 2.0),
        "a new layer is one world unit a pixel"
    );
    let corner = entity.transform.to_world(CanvasPoint::new(-0.5, -0.5));
    assert_eq!(
        corner,
        CanvasPoint::new(corner.x.round(), corner.y.round()),
        "on whole units"
    );
    assert_eq!(
        entity
            .transform
            .pixel_at(3, 2, CanvasPoint::new(corner.x + 2.5, corner.y + 0.5)),
        (2, 0)
    );
    assert_eq!(
        entity
            .transform
            .pixel_at(3, 2, CanvasPoint::new(corner.x - 0.5, corner.y)),
        (-1, 0)
    );

    let mut scaled = entity.clone();
    scaled.transform = CanvasTransform::new(
        CanvasPoint::new(100.5, 40.0),
        CanvasPoint::new(30.0, 10.0),
        0.4,
    );
    let scaled_content = canvas_run(
        &content,
        InfiniteCanvasOperation::Update {
            entities: vec![scaled],
        },
    );
    let held = scaled_content.root().entities()[0].clone();
    assert_eq!(
        held.transform,
        CanvasTransform::new(
            CanvasPoint::new(100.5, 40.0),
            CanvasPoint::new(30.0, 10.0),
            0.0
        ),
        "a layer scales like an image, but never turns"
    );
    let raster = Canvas::raster(&scaled_content, id).expect("a raster layer");
    assert_eq!(
        (raster.width(), raster.height()),
        (3, 2),
        "and keeps its pixels"
    );
    assert_eq!(
        held.transform
            .pixel_at(3, 2, CanvasPoint::new(85.5 + 25.0, 35.0 + 6.0)),
        (2, 1),
        "a point maps to the scaled pixel under it"
    );
}
