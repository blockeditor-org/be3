use crate::Merge;
use crate::pixel_art::{
    PixelArtAnchor, PixelArtContent, PixelArtOperation, PixelColor, PixelUpdate,
};
use be_commit::MergeResult;

const RED: PixelColor = PixelColor::new(255, 0, 0, 255);
const BLUE: PixelColor = PixelColor::new(0, 0, 255, 255);

fn run(content: &PixelArtContent, operation: PixelArtOperation) -> PixelArtContent {
    let mut changed = content.clone();
    let edit = content.root().edit_for(&operation);
    changed.apply(&edit);
    changed
}

#[test]
fn pixel_art_fills_resizes_and_keeps_a_paint_made_during_a_resize() {
    let blank = PixelArtContent::default();
    let art = blank.root().artwork();
    assert_eq!((art.width(), art.height()), (32, 32));
    assert_eq!(art.palette().len(), 16);

    let walled = run(
        &blank,
        PixelArtOperation::Paint {
            pixels: (0..32)
                .map(|y| PixelUpdate {
                    x: 4,
                    y,
                    color: RED,
                })
                .collect(),
        },
    );
    let filled = run(
        &walled,
        PixelArtOperation::Fill {
            x: 0,
            y: 0,
            color: BLUE,
        },
    );
    let art = filled.root().artwork();
    assert_eq!(art.pixel(3, 31), Some(BLUE));
    assert_eq!(art.pixel(4, 0), Some(RED));
    assert_eq!(art.pixel(5, 0), Some(PixelColor::TRANSPARENT));

    let ours = run(
        &filled,
        PixelArtOperation::Resize {
            width: 16,
            height: 16,
            anchor: PixelArtAnchor::BottomRight,
        },
    );
    let theirs = run(
        &filled,
        PixelArtOperation::Paint {
            pixels: vec![PixelUpdate {
                x: 20,
                y: 20,
                color: RED,
            }],
        },
    );
    let MergeResult::Clean(merged) = PixelArtContent::merge3(&filled, &ours, &theirs) else {
        panic!("a resize and a paint touch different things");
    };
    let art = merged.root().artwork();
    assert_eq!((art.width(), art.height()), (16, 16));
    assert_eq!(art.pixel(4, 4), Some(RED), "the paint moved with the crop");

    let cleared = run(&merged, PixelArtOperation::Clear);
    assert!(
        cleared
            .root()
            .artwork()
            .rgba_bytes()
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(
        run(&cleared, PixelArtOperation::Clear),
        cleared,
        "clearing a clear canvas changes nothing"
    );
}
