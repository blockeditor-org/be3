use super::*;

#[test]
fn clicking_a_card_selects_it() {
    let (mut editor, _host) = editor();

    editor.click("pan_zoom.card.0");
    editor.run();

    assert_eq!(
        shown(&mut editor, "pan_zoom.selected"),
        "\"Selected Origin\""
    );
    editor.snapshot("clicking_a_card_selects_it");
}
