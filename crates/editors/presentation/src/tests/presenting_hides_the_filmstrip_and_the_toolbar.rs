use super::*;

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
    assert!(presenting.left() < editing.left());
    assert!(presenting.top() < editing.top());

    test.hover("presentation.playback");
    test.run();
    assert_eq!(detail(&test, "presentation.playback.position"), "1 / 3");

    test.key_press(Key::ArrowRight);
    test.run();
    assert_eq!(detail(&test, "presentation.playback.position"), "2 / 3");
}
