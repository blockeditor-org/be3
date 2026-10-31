use super::*;
use crate::CanvasRasterAnchor;

#[test]
fn resizing_a_raster_layer_keeps_its_pixels_where_they_were() {
    let (content, id) = raster_layer(&CanvasContent::default(), 32, 32);
    let content = edited(&content, [paint(&content, id, 31, 31, RED)]);
    let world_of = |content: &CanvasContent, x: u16, y: u16| {
        let entity = content
            .root()
            .entities()
            .into_iter()
            .find(|entity| entity.id == id)
            .expect("the layer");
        let raster = Canvas::raster(content, id).expect("a raster layer");
        entity.transform.to_world(CanvasPoint::new(
            (f32::from(x) + 0.5) / f32::from(raster.width()) - 0.5,
            (f32::from(y) + 0.5) / f32::from(raster.height()) - 0.5,
        ))
    };
    let before = world_of(&content, 31, 31);

    for (anchor, at) in [
        (CanvasRasterAnchor::BottomRight, (19, 9)),
        (CanvasRasterAnchor::TopLeft, (31, 31)),
        (CanvasRasterAnchor::Center, (25, 20)),
    ] {
        let resized = canvas_run(
            &content,
            InfiniteCanvasOperation::Resize {
                id,
                width: 20,
                height: 10,
                anchor,
            },
        );
        let raster = Canvas::raster(&resized, id).expect("a raster layer");
        assert_eq!((raster.width(), raster.height()), (20, 10));
        let grown = canvas_run(
            &resized,
            InfiniteCanvasOperation::Resize {
                id,
                width: 40,
                height: 40,
                anchor,
            },
        );
        let raster = Canvas::raster(&grown, id).expect("a raster layer");
        let kept = (0..40u16)
            .flat_map(|y| (0..40u16).map(move |x| (x, y)))
            .find(|(x, y)| raster.pixel(*x, *y) == Some(RED));
        match anchor {
            CanvasRasterAnchor::BottomRight => {
                assert_eq!(world_of(&resized, at.0, at.1), before);
                let (x, y) = kept.expect("the red pixel survives a crop at its corner");
                assert_eq!(world_of(&grown, x, y), before, "{anchor:?}");
            }
            _ => assert!(kept.is_none(), "{anchor:?} cropped the red pixel away"),
        }
    }
}
