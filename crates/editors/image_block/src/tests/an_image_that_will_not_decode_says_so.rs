use super::*;

#[test]
fn an_image_that_will_not_decode_says_so() {
    let (mut editor, block) = editor(b"not an image".to_vec());

    editor.run();
    editor.run();

    assert!(matches!(
        block.read().unwrap().metadata(),
        block_client::blocks::image::ImageMetadata::Failed(_)
    ));
}
