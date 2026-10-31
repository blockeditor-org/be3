use super::*;
use crate::{DEFAULT_PIXEL_SIZE, snap_to_pixels};

#[test]
fn a_raster_layer_sits_on_the_canvas_wide_pixel_grid() {
    let blank = CanvasContent::default();
    assert_eq!(blank.root().resolved_pixel_size(), DEFAULT_PIXEL_SIZE);
    let size = DEFAULT_PIXEL_SIZE;

    let layer = CanvasEntity::new(
        CanvasShape::Raster,
        CanvasTransform::new(
            CanvasPoint::new(13.0, -5.0),
            CanvasPoint::new(size * 3.4, size * 2.0),
            0.7,
        ),
    );
    let id = layer.id;
    let content = canvas_run(
        &blank,
        InfiniteCanvasOperation::Add {
            entity: Box::new(layer),
        },
    );
    let raster = Canvas::raster(&content, id).expect("a raster layer");
    assert_eq!((raster.width(), raster.height()), (3, 2));
    let entity = content.root().entities()[0].clone();
    assert_eq!(entity.transform.rotation, 0.0);
    assert_eq!(
        entity.transform.size,
        CanvasPoint::new(size * 3.0, size * 2.0)
    );
    let corner = entity.transform.to_world(CanvasPoint::new(-0.5, -0.5));
    assert_eq!(snap_to_pixels(size, corner), corner);
    assert_eq!(
        entity.transform.pixel_at(
            3,
            2,
            CanvasPoint::new(corner.x + size * 2.5, corner.y + size * 0.5)
        ),
        (2, 0)
    );
    assert_eq!(
        entity
            .transform
            .pixel_at(3, 2, CanvasPoint::new(corner.x - size * 0.5, corner.y)),
        (-1, 0)
    );

    let mut moved = entity.clone();
    moved.transform.center = CanvasPoint::new(entity.transform.center.x + 3.0, 100.0);
    moved.transform.size = CanvasPoint::new(500.0, 500.0);
    let moved_content = canvas_run(
        &content,
        InfiniteCanvasOperation::Update {
            entities: vec![moved],
        },
    );
    let held = moved_content.root().entities()[0].clone();
    assert_eq!(
        held.transform.size, entity.transform.size,
        "a move never stretches pixels"
    );
    let corner = held.transform.to_world(CanvasPoint::new(-0.5, -0.5));
    assert_eq!(snap_to_pixels(size, corner), corner);

    let doubled = edited(&content, [Canvas::set_pixel_size(&content, size * 2.0)]);
    assert_eq!(doubled.root().resolved_pixel_size(), size * 2.0);
    let held = doubled.root().entities()[0].clone();
    assert_eq!(
        held.transform.size,
        CanvasPoint::new(size * 6.0, size * 4.0)
    );
    let corner = held.transform.to_world(CanvasPoint::new(-0.5, -0.5));
    assert_eq!(snap_to_pixels(size * 2.0, corner), corner);
    assert!(Canvas::set_pixel_size(&doubled, 0.0).0.is_empty());
    assert!(Canvas::set_pixel_size(&doubled, size * 2.0).0.is_empty());
}
