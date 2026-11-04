use super::*;

#[test]
fn a_card_shows_only_the_stage() {
    let (mut test, editor) = editor(3);
    test.run();
    assert!(test.shown("presentation.toolbar"));

    test.set_chrome(false);
    test.run();
    assert!(
        !test.shown("presentation.toolbar") && !test.shown("presentation.filmstrip"),
        "a deck drawn as a card declares no dock, only its stage"
    );
    assert_eq!(test.rect_of("presentation.stage"), editor.content_rect());

    test.set_chrome(true);
    test.run();
    assert!(test.shown("presentation.toolbar") && test.shown("presentation.filmstrip"));
}
