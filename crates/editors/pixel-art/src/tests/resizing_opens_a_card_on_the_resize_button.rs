use super::*;

#[test]
fn resizing_opens_a_card_on_the_resize_button() {
    let mut editor = editor();

    editor.click("pixel-art.resize");
    editor.run();

    assert!(editor.shown("pixel-art.resize.apply"));
    let button = editor.rect_of("pixel-art.resize");
    let width = editor.rect_of("pixel-art.resize-width");
    assert!(
        width.top() > button.bottom(),
        "the card hangs below the button that opened it: {width:?} under {button:?}"
    );
    editor.snapshot("resizing_opens_a_card_on_the_resize_button");
}
