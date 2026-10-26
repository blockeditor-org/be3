use super::*;
use crate::pixel_art::{PixelArtContent, PixelArtOperation, PixelColor, PixelUpdate};

const RED: PixelColor = PixelColor::new(255, 0, 0, 255);
const BLUE: PixelColor = PixelColor::new(0, 0, 255, 255);

fn paint(content: &PixelArtContent, x: u16, y: u16, color: PixelColor) -> Edit {
    content.root().edit_for(&PixelArtOperation::Paint {
        pixels: vec![PixelUpdate { x, y, color }],
    })
}

#[test]
fn undoing_a_clear_keeps_a_pixel_painted_since() {
    let blank = PixelArtContent::default();
    let mut content = edited(&blank, [paint(&blank, 0, 0, RED)]);
    let second = paint(&content, 1, 0, RED);
    content.apply(&second);

    let clear = content.root().edit_for(&PixelArtOperation::Clear);
    let step = undone(&mut content, clear);
    let repaint = paint(&content, 1, 0, BLUE);
    content.apply(&repaint);
    reverted(&mut content, &step);

    let art = content.root().artwork();
    assert_eq!(art.pixel(0, 0), Some(RED));
    assert_eq!(art.pixel(1, 0), Some(BLUE));
    assert_eq!(art.pixel(2, 0), Some(PixelColor::TRANSPARENT));
}
