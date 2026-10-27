use super::*;
use block_editor_beui::beui::Modifiers;

#[test]
fn presenting_hides_the_filmstrip_and_the_toolbar() {
    let (mut test, editor) = editor(3);
    test.run();
    assert!(test.shown("presentation.add"));
    let editing = editor.content_rect();

    editor.host().set_presenting(true);
    test.run();

    assert!(!test.shown("presentation.add"));
    let presenting = editor.content_rect();
    assert!(presenting.top() < editing.top());
    let window = test.rect();
    assert_eq!(
        presenting.center().x,
        window.center().x,
        "the slide is centred in the whole window, not beside a hidden filmstrip"
    );
    test.snapshot("presenting_hides_the_filmstrip_and_the_toolbar");

    test.hover("presentation.playback");
    assert_eq!(detail(&test, "presentation.playback.position"), "1 / 3");

    test.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(detail(&test, "presentation.playback.position"), "2 / 3");
}
