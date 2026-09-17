use super::*;

#[test]
fn the_filmstrip_stays_while_a_slide_holds_the_frame() {
    let (mut test, editor, block) = editor(3);
    test.run();

    editor.host().set_chrome_shown(false);
    test.run();

    assert!(
        test.shown("presentation.add"),
        "the toolbar should stay while a slide is edited"
    );
    for id in slide_ids(&block) {
        assert!(
            test.shown(&format!("presentation.slide.{id}")),
            "slide {id} left the filmstrip while a slide was edited"
        );
    }
}
