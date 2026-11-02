use super::*;

const SHAPE: &str = "GenericContainer #infinite-canvas.entity.";

#[test]
fn a_canvas_in_a_canvas_keeps_to_its_card_and_closes_when_pressed_outside() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    let (x, y, width, height) = app.place("Pane \"Canvas\"");
    let full = format!("Pane \"Canvas\" at {x},{y} size {width}x{height}");
    add_to_canvas(&mut app, "Canvas");
    let card = app.place(SHAPE);

    let inside = format!("{:.0},{:.0}", card.0 + card.2 / 2.0, card.1 + card.3 * 0.6);
    app.run(&["click", &inside]);
    assert_eq!(app.count(&full), 2, "the inner canvas takes the frame");
    app.click("#infinite-canvas.dock.tool.Rectangle");
    app.record();

    let outside = format!("{:.0},{:.0}", card.0 - 60.0, card.1 + card.3 / 2.0);
    let further = format!("{:.0},{:.0}", card.0 - 20.0, card.1 + card.3 / 2.0 + 60.0);
    app.run(&["drag", &outside, &further]);
    assert_eq!(
        app.count(&full),
        1,
        "a press outside the inner canvas gives the frame back"
    );
    assert_eq!(app.count(SHAPE), 1, "and draws nothing, in either canvas");
    app.snapshot("a_canvas_in_a_canvas_keeps_to_its_card_and_closes_when_pressed_outside");
    app.close();
}
