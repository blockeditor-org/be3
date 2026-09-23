use super::*;

#[test]
fn an_image_that_will_not_decode_says_so() {
    let mut editor = editor(b"not an image".to_vec());

    editor.run();
    editor.run();

    let header = editor.content::<ImageContent>(None).header().clone();
    assert!(header.failure.is_some());
}
