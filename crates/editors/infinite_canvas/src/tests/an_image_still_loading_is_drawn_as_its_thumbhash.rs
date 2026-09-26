use super::*;
use block_editor_plugin::be_block::{BlockContent, ImageContent};
use block_editor_plugin::beui::Image;

#[test]
fn an_image_still_loading_is_drawn_as_its_thumbhash() {
    let picture = Uuid::new_v4();
    let mut placed = entity(Uuid::from_u128(1));
    placed.kind = CanvasEntityKind::DirectEditor {
        block_id: picture,
        scale: 1.0,
    };
    placed.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(240.0, 180.0), 0.0);
    let pixels: Vec<u8> = (0..32 * 24)
        .flat_map(|at: u32| {
            let (x, y) = (at % 32, at / 32);
            [(x * 8) as u8, (y * 10) as u8, 160, 255]
        })
        .collect();
    let mut info = BlockInfo::new(picture, ImageContent::CONTENT_TYPE, BlockParent::Detached);
    info.thumbhash = Some(Image::from_rgba(32, 24, pixels).thumbhash());
    let mut editor = open(
        &Canvas::with_entities([placed.clone()], None),
        false,
        &[info],
    );
    editor.run();

    assert!(
        editor.shown(&format!("infinite-canvas.entity.{}", placed.id)),
        "the image has an item on the canvas while its editor is not yet available"
    );
    editor.snapshot("an_image_still_loading_is_drawn_as_its_thumbhash");
}
