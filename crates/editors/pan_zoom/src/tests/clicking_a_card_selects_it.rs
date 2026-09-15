use super::*;

#[test]
fn clicking_a_card_selects_it() {
    let (mut test, _editor) = editor();

    test.click("pan_zoom.card.0");
    test.run();

    assert_eq!(shown(&mut test, "pan_zoom.selected"), "\"Selected Origin\"");
    test.snapshot("clicking_a_card_selects_it");
}
