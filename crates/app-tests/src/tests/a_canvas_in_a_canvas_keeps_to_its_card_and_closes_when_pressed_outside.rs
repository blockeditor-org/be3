use super::*;

const SHAPE: &str = "GenericContainer #infinite-canvas.entity.";
const FULL: &str = "Pane \"Canvas\" at 253,53 size 837x606";

#[test]
fn a_canvas_in_a_canvas_keeps_to_its_card_and_closes_when_pressed_outside() {
    let mut app = AppTest::start();
    add_root_block(&mut app, "Canvas");
    add_to_canvas(&mut app, CANVAS_TILE);
    let card = app.place(SHAPE);
    app.crop("#workspace.block");

    let inside = format!("{:.0},{:.0}", card.0 + card.2 / 2.0, card.1 + card.3 * 0.6);
    app.run(&["click", &inside]);
    assert_eq!(app.count(FULL), 2, "the inner canvas takes the frame");
    app.click("#infinite-canvas.dock.tool.Rectangle");
    app.record();

    let outside = format!("{:.0},{:.0}", card.0 - 60.0, card.1 + card.3 / 2.0);
    let further = format!("{:.0},{:.0}", card.0 - 20.0, card.1 + card.3 / 2.0 + 60.0);
    app.run(&["drag", &outside, &further]);
    assert_eq!(app.count(FULL), 1, "a press outside the inner canvas gives the frame back");
    assert_eq!(app.count(SHAPE), 1, "and draws nothing, in either canvas");
    app.snapshot("a_canvas_in_a_canvas_keeps_to_its_card_and_closes_when_pressed_outside");
    app.close();
}
