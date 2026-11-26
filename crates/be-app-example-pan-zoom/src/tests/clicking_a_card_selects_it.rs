use super::*;

#[test]
fn clicking_a_card_selects_it() {
    let (test, _editor) = editor();
    let mut test = test.in_viewport();

    test.click("pan_zoom.card.0");

    assert_eq!(shown(&mut test, "pan_zoom.selected"), "\"Selected Origin\"");
    test.snapshot("clicking_a_card_selects_it");
}
