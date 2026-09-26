use block_editor_plugin::be_block::BlockContent;
use block_editor_plugin::be_block::ImageContent;
use block_editor_plugin::{BlockInfo, BlockParent, Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use image::ImageEncoder;
use uuid::Uuid;

use crate::app::ImageApp;

mod a_decoded_image_is_painted_at_its_shape;
mod an_image_still_loading_shows_its_thumbhash;
mod an_image_that_will_not_decode_says_so;

fn editor(data: Vec<u8>) -> ContentHarness<ImageApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = ContentHarness::new(BeuiTest::new(editor), host);
    editor.hold(None, ImageContent::from_file("picture.png", data));
    editor.run();
    editor.run();
    editor
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut buffer = Vec::new();
    let pixels: Vec<u8> = (0..width * height)
        .flat_map(|index| {
            let shade = (index % 256) as u8;
            [shade, 255 - shade, 128, 255]
        })
        .collect();
    image::codecs::png::PngEncoder::new(&mut buffer)
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .expect("the test image encodes");
    buffer
}
