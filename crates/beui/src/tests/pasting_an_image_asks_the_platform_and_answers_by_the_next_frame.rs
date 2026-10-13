use super::*;
use crate::PickedFile;

#[test]
fn pasting_an_image_asks_the_platform_and_answers_by_the_next_frame() {
    let mut intaken = Intaken::new();
    let image = PickedFile {
        name: "Pasted Image.png".to_owned(),
        data: vec![0x89, b'P', b'N', b'G'],
    };
    intaken.platform.clipboard_image = Some(image.clone());

    let id = intaken.runner.context().paste_image();
    intaken.frame(1.0, []);
    intaken.frame(1.0, []);
    assert_eq!(
        intaken.runner.context().take_image_paste(id),
        Some(Ok(Some(image)))
    );
    assert_eq!(intaken.runner.context().take_image_paste(id), None);

    intaken.platform.clipboard_image = None;
    let empty = intaken.runner.context().paste_image();
    intaken.frame(1.0, []);
    intaken.frame(1.0, []);
    assert_eq!(
        intaken.runner.context().take_image_paste(empty),
        Some(Ok(None)),
        "a clipboard without an image answers with nothing rather than never"
    );
}
