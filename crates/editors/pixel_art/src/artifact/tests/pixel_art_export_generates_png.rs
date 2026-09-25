use block_editor_plugin::be_block::PixelArtContent;
use block_editor_plugin::be_block::pixel_art::{PixelArtOperation, PixelColor, PixelUpdate};

use super::generate_initial;

#[test]
fn pixel_art_export_generates_png() {
    let blank = PixelArtContent::default();
    let mut content = blank.clone();
    content.apply(&blank.root().edit_for(&PixelArtOperation::Paint {
        pixels: vec![PixelUpdate {
            x: 0,
            y: 0,
            color: PixelColor::new(12, 34, 56, 78),
        }],
    }));
    let art = content.root().artwork();

    let generated = generate_initial(&art, "Sprite").unwrap();
    let decoded = image::load_from_memory(generated.data())
        .unwrap()
        .into_rgba8();

    assert_eq!(generated.header().source_name, "Sprite Export");
    assert_eq!(decoded.dimensions(), (32, 32));
    assert_eq!(decoded.get_pixel(0, 0).0, [12, 34, 56, 78]);
    assert_eq!(decoded.get_pixel(1, 0).0, [0, 0, 0, 0]);
}
