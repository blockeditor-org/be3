use std::sync::Arc;

use block_client::blocks::image::Image;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use image::ImageEncoder;
use uuid::Uuid;

use crate::app::ImageApp;

mod a_decoded_image_is_painted_at_its_shape;
mod an_image_that_will_not_decode_says_so;

fn editor(data: Vec<u8>) -> (BeuiTest<ImageApp>, BlockHandle<Image>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Image::new("picture.png".to_owned(), data));
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::new(editor);
    editor.run();
    editor.run();
    (editor, block)
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
