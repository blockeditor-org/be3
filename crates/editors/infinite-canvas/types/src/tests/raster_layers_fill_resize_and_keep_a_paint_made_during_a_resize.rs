use super::*;
use crate::{CanvasRasterAnchor, MAX_RASTER_PALETTE_COLORS};

#[test]
fn raster_layers_fill_resize_and_keep_a_paint_made_during_a_resize() {
    let blank = CanvasContent::default();
    assert_eq!(Canvas::palette_in(&blank).len(), 16);
    let (blank, id) = raster_layer(&blank, 32, 32);
    let raster = Canvas::raster(&blank, id).expect("a raster layer");
    assert_eq!((raster.width(), raster.height()), (32, 32));
    assert_eq!(raster.rgba_bytes().len(), 32 * 32 * 4);

    let walled = canvas_run(
        &blank,
        InfiniteCanvasOperation::Paint {
            id,
            pixels: (0..32)
                .map(|y| CanvasPixelUpdate { x: 4, y, color: RED })
                .collect(),
        },
    );
    let filled = canvas_run(
        &walled,
        InfiniteCanvasOperation::Fill {
            id,
            x: 0,
            y: 0,
            color: BLUE,
        },
    );
    let raster = Canvas::raster(&filled, id).expect("a raster layer");
    assert_eq!(raster.pixel(3, 31), Some(BLUE));
    assert_eq!(raster.pixel(4, 0), Some(RED));
    assert_eq!(raster.pixel(5, 0), Some(CanvasPixel::TRANSPARENT));

    let recolored = canvas_run(
        &filled,
        InfiniteCanvasOperation::ReplaceColor {
            id,
            from: BLUE,
            to: RED,
        },
    );
    let raster = Canvas::raster(&recolored, id).expect("a raster layer");
    assert_eq!(raster.pixel(3, 31), Some(RED));
    assert_eq!(raster.pixel(5, 0), Some(CanvasPixel::TRANSPARENT));

    let ours = canvas_run(
        &filled,
        InfiniteCanvasOperation::Resize {
            id,
            width: 16,
            height: 16,
            anchor: CanvasRasterAnchor::BottomRight,
        },
    );
    let theirs = edited(&filled, [paint(&filled, id, 20, 20, RED)]);
    let (merged, conflicts) = merged(&filled, &ours, &theirs);
    assert_eq!(conflicts, 0, "a resize and a paint touch different things");
    let raster = Canvas::raster(&merged, id).expect("a raster layer");
    assert_eq!((raster.width(), raster.height()), (16, 16));
    assert_eq!(raster.pixel(4, 4), Some(RED), "the paint moved with the crop");

    let cleared = canvas_run(&merged, InfiniteCanvasOperation::Clear { id });
    let raster = Canvas::raster(&cleared, id).expect("a raster layer");
    assert!(raster.rgba_bytes().iter().all(|byte| *byte == 0));
    assert!(
        Canvas::edit_in(&cleared, &InfiniteCanvasOperation::Clear { id })
            .0
            .is_empty(),
        "clearing a clear layer changes nothing"
    );

    let many: Vec<CanvasPixel> = (0..40).map(|red| CanvasPixel::new(red, 0, 0, 255)).collect();
    let palette = canvas_run(
        &cleared,
        InfiniteCanvasOperation::SetPalette {
            colors: [vec![RED, RED], many].concat(),
        },
    );
    let colors = Canvas::palette_in(&palette);
    assert_eq!(colors.len(), MAX_RASTER_PALETTE_COLORS);
    assert_eq!(colors[0], RED);
    assert_ne!(colors[1], RED, "a repeated colour is kept once");
}
