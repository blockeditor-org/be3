use block_editor_plugin::be_block::PixelArtContent;
use block_editor_plugin::be_block::pixel_art::{PixelArtOperation, PixelColor, PixelUpdate};

use super::{ImageSettings, generate};

#[test]
fn scale_setting_magnifies_the_export() {
    let blank = PixelArtContent::default();
    let mut content = blank.clone();
    content.apply(&blank.root().edit_for(&PixelArtOperation::Paint {
        pixels: vec![PixelUpdate {
            x: 1,
            y: 0,
            color: PixelColor::new(12, 34, 56, 78),
        }],
    }));
    let art = content.root().artwork();

    let generated = generate(&art, "Sprite", &ImageSettings { scale: 3 }).unwrap();
    let decoded = image::load_from_memory(generated.data())
        .unwrap()
        .into_rgba8();

    assert_eq!(decoded.dimensions(), (96, 96));

    for x in 3..6 {
        for y in 0..3 {
            assert_eq!(decoded.get_pixel(x, y).0, [12, 34, 56, 78], "{x},{y}");
        }
    }
    assert_eq!(decoded.get_pixel(2, 0).0, [0, 0, 0, 0]);
    assert_eq!(decoded.get_pixel(6, 0).0, [0, 0, 0, 0]);
    assert_eq!(decoded.get_pixel(3, 3).0, [0, 0, 0, 0]);
}
