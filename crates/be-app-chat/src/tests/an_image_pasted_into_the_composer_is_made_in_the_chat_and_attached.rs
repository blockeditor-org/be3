use be_sdk_beui::beui::Input;
use be_sdk_beui::beui::reactive::{ClipItem, formats};

use super::*;

#[test]
fn an_image_pasted_into_the_composer_is_made_in_the_chat_and_attached() {
    let mut chat = Harness::new(&[]);
    chat.click("chat.draft");

    // Text pastes as text, and makes nothing.
    chat.editor.step(vec![Input::Paste(ClipItem::new(
        formats::TEXT,
        b"Look".to_vec(),
    ))]);
    chat.run();
    assert!(chat.editor.store().created().is_empty());
    assert!(chat.content().drafts_of(ME).is_empty());

    let png = b"\x89PNG\r\n\x1a\n a picture".to_vec();
    chat.editor
        .step(vec![Input::Paste(ClipItem::new(formats::PNG, png))]);
    chat.run();
    let made = chat.editor.store().created();
    assert_eq!(made.len(), 1, "the image is made as a block");
    let image = made[0];
    assert_eq!(
        chat.editor
            .store()
            .block(image)
            .and_then(|info| info.parent),
        Some(BlockParent::Block(CHAT)),
        "in the chat"
    );
    assert_eq!(chat.content().drafts_of(ME), [image]);
    assert!(chat.editor.shown(&format!("chat.attaching.{image}")));

    chat.click("chat.send");
    chat.editor.advance(Duration::from_millis(500));
    chat.run();
    assert_eq!(
        chat.bodies(),
        ["Look"],
        "the text typed goes with the image"
    );
}
