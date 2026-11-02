use super::*;

#[test]
fn the_editor_opens_with_its_cards_fitted_into_the_view() {
    let (test, _editor) = editor();
    let mut test = test.in_viewport();
    test.run();

    let stage = test.content_rect().expect("the editor reports its stage");
    let origin = test.rect_of("pan_zoom.card.0");
    let corner = test.rect_of("pan_zoom.card.6");
    let cards = origin.union(corner);
    assert!(
        stage.expand(0.5).contains_rect(cards),
        "every card is on screen, {cards:?} in {stage:?}"
    );
    assert_eq!(shown(&mut test, "pan_zoom.zoom"), "\"Zoom 66%\"");
    test.snapshot("the_editor_opens_with_its_cards_fitted_into_the_view");
}
