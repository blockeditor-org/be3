use super::*;
use crate::CanvasRasterAnchor;

#[test]
fn a_pixel_painted_outside_the_other_sides_crop_counts_as_a_conflict() {
    let (blank, id) = raster_layer(&CanvasContent::default(), 32, 32);
    let base = edited(
        &blank,
        [paint(&blank, id, 0, 0, CanvasPixel::new(0, 0, 0, 255))],
    );
    let ours = canvas_run(
        &base,
        InfiniteCanvasOperation::Resize {
            id,
            width: 8,
            height: 8,
            anchor: CanvasRasterAnchor::TopLeft,
        },
    );
    let theirs = edited(&base, [paint(&base, id, 20, 20, RED)]);

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    let raster = Canvas::raster(&merged, id).expect("a raster layer");
    assert_eq!((raster.width(), raster.height()), (8, 8));
    assert_eq!(conflicts, 1);
}
