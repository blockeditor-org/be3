use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const LANDSCAPE_PHONE: Vec2 = Vec2::new(844.0, 390.0);

fn fitted(test: &BeuiTest<PanZoomRoot>) {
    let stage = test.content_rect().expect("the editor reports its stage");
    let cards = test
        .rect_of("pan_zoom.card.0")
        .union(test.rect_of("pan_zoom.card.6"));
    assert!(
        stage.expand(0.5).contains_rect(cards),
        "every card is fitted into the stage, {cards:?} in {stage:?}"
    );
}

#[test]
fn on_a_phone_the_cards_are_gathered_beside_the_fitted_canvas() {
    let (test, _editor) = editor();
    let mut test = test.in_viewport().with_size(PHONE);
    test.settle();
    let stage = test.content_rect().expect("the editor reports its stage");
    let gathered = test.rect_of("dock.compact");
    assert!(
        stage.bottom() <= gathered.top() + 1.0,
        "the cards are gathered below the canvas, {stage:?} {gathered:?}"
    );
    fitted(&test);
    test.snapshot("on_a_phone_the_cards_are_gathered_below_the_fitted_canvas");

    let splitter = test.rect_of("dock.compact.splitter").center();
    test.drag(splitter, splitter - Vec2::new(0.0, 200.0));
    test.settle();
    assert!(
        test.content_rect()
            .expect("the editor reports its stage")
            .height()
            < stage.height() - 100.0,
        "dragging the splitter up shortens the canvas"
    );
    fitted(&test);

    test.set_size(LANDSCAPE_PHONE);
    test.settle();
    let stage = test.content_rect().expect("the editor reports its stage");
    let gathered = test.rect_of("dock.compact");
    assert!(
        stage.right() <= gathered.left() + 1.0,
        "on a phone on its side the cards are gathered right of the canvas, {stage:?} {gathered:?}"
    );
    fitted(&test);
    test.snapshot("on_a_phone_on_its_side_the_cards_are_gathered_right_of_the_fitted_canvas");
}
