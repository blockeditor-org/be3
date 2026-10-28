use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockInfo, BlockParent, EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use block_ui_test::prelude::*;
use image::ImageEncoder;
use uuid::Uuid;

use crate::app::ImageApp;
use image_block_types::{ImageContent, ImageFile};

mod a_decoded_image_is_painted_at_its_shape;
mod a_narrow_image_folds_its_sidebar_under_the_picture;
mod a_new_image_asks_for_its_file_in_a_dialog_it_draws_and_closes;
mod an_image_still_loading_shows_its_thumbhash;
mod an_image_that_will_not_decode_says_so;

fn editor(data: Vec<u8>) -> BeuiTest<ImageApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = OwnedEditor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor);
    editor.hold(None, ImageFile::from_file("picture.png", data));
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
