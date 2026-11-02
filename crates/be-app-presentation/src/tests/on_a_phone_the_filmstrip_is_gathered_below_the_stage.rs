use super::*;
use be_sdk_beui::beui::Vec2;

#[test]
fn on_a_phone_the_filmstrip_is_gathered_below_the_stage() {
    let (test, _editor) = editor(3);
    let mut test = test.with_size(Vec2::new(390.0, 760.0));
    test.settle();

    assert!(
        test.shown("presentation.stage") && test.shown("presentation.filmstrip"),
        "the deck shows its stage with the filmstrip gathered below it"
    );
    assert!(
        test.rect_of("presentation.stage").bottom() <= test.rect_of("dock.compact").top() + 1.0,
        "the filmstrip sits below the stage"
    );
    test.snapshot("on_a_phone_the_filmstrip_is_gathered_below_the_stage");
}
