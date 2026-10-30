use super::*;
use block_editor_beui::beui::Vec2;

#[test]
fn on_a_phone_the_filmstrip_is_a_page_of_its_own() {
    let (test, _editor) = editor(3);
    let mut test = test.with_size(Vec2::new(390.0, 760.0));
    test.settle();

    assert!(
        test.shown("presentation.stage") && !test.shown("presentation.filmstrip"),
        "the stacked deck opens on the stage, with the filmstrip a page of its own"
    );
    test.snapshot("on_a_phone_the_filmstrip_is_a_page_of_its_own");
}
