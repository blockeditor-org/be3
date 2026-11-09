use super::*;
use be_sdk_beui::beui::Vec2;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);

#[test]
fn on_a_phone_the_notes_are_a_pane_beside_the_filmstrip() {
    let (mut test, _editor) = editor(3);
    test.settle();
    let stage = test.rect_of("presentation.stage");
    let notes = test.rect_of("presentation.notes.add");
    assert!(
        stage.bottom() <= notes.top() + 1.0,
        "on a desktop the notes sit under the slide, {stage:?} {notes:?}"
    );
    test.snapshot("on_a_desktop_the_notes_sit_under_the_slide");

    test.set_size(PHONE);
    test.settle();
    test.click("dock.tab.notes");
    test.settle();
    let stage = test.rect_of("presentation.stage");
    let gathered = test.rect_of("dock.compact");
    assert!(
        test.shown("presentation.notes.add") && stage.bottom() <= gathered.top() + 1.0,
        "on a phone the notes are a pane gathered below the slide, {stage:?} {gathered:?}"
    );
    test.snapshot("on_a_phone_the_notes_are_a_pane_beside_the_filmstrip");
}
