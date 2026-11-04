use super::*;
use be_sdk_beui::beui::Vec2;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const LANDSCAPE_PHONE: Vec2 = Vec2::new(844.0, 390.0);

#[test]
fn on_a_phone_the_filmstrip_is_gathered_beside_the_slide() {
    let (test, _editor) = editor(3);
    let mut test = test.with_size(PHONE);
    test.settle();
    let stage = test.rect_of("presentation.stage");
    let gathered = test.rect_of("dock.compact");
    assert!(
        test.shown("presentation.filmstrip") && stage.bottom() <= gathered.top() + 1.0,
        "the filmstrip is gathered below the slide, {stage:?} {gathered:?}"
    );
    assert!(test.shown("presentation.toolbar"));
    test.snapshot("on_a_phone_the_filmstrip_is_gathered_below_the_slide");

    test.set_size(LANDSCAPE_PHONE);
    test.settle();
    let stage = test.rect_of("presentation.stage");
    let gathered = test.rect_of("dock.compact");
    assert!(
        test.shown("presentation.filmstrip") && stage.right() <= gathered.left() + 1.0,
        "on a phone on its side the filmstrip is gathered right of the slide, {stage:?} {gathered:?}"
    );
    test.snapshot("on_a_phone_on_its_side_the_filmstrip_is_gathered_right_of_the_slide");
}
