use super::*;
use crate::pixel_art::{
    PixelArtAnchor, PixelArtContent, PixelArtOperation, PixelColor, PixelUpdate,
};

#[test]
#[ignore = "a merge drops a pixel painted outside the other side's crop without counting a conflict"]
fn a_pixel_painted_outside_the_other_sides_crop_counts_as_a_conflict() {
    let blank = PixelArtContent::default();
    let base = edited(
        &blank,
        [blank.root().edit_for(&PixelArtOperation::Paint {
            pixels: vec![PixelUpdate {
                x: 0,
                y: 0,
                color: PixelColor::new(0, 0, 0, 255),
            }],
        })],
    );
    let ours = edited(
        &base,
        [base.root().edit_for(&PixelArtOperation::Resize {
            width: 8,
            height: 8,
            anchor: PixelArtAnchor::TopLeft,
        })],
    );
    let theirs = edited(
        &base,
        [base.root().edit_for(&PixelArtOperation::Paint {
            pixels: vec![PixelUpdate {
                x: 20,
                y: 20,
                color: PixelColor::new(255, 0, 0, 255),
            }],
        })],
    );

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    let art = merged.root().artwork();
    assert_eq!((art.width(), art.height()), (8, 8));
    assert_eq!(conflicts, 1);
}
