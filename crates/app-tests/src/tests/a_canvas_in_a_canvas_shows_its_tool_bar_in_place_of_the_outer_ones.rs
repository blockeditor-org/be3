use super::*;

const SHAPE: &str = "GenericContainer #infinite-canvas.entity.";
const GRIP: &str = "#infinite-canvas.dock.grip";
const BAR: &str = "GenericContainer #infinite-canvas.dock at";

#[test]
fn a_canvas_in_a_canvas_shows_its_tool_bar_in_place_of_the_outer_ones() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    add_to_canvas(&mut app, "Canvas");
    app.click(GRIP);
    app.click("#grip.dock.bar.beside.left");
    app.run(&["settle"]);
    let outer = app.place(BAR);
    let card = app.place(SHAPE);

    let inside = format!("{:.0},{:.0}", card.0 + card.2 / 2.0, card.1 + card.3 * 0.6);
    app.run(&["click", &inside]);
    app.run(&["settle"]);
    assert_eq!(app.count(GRIP), 1, "only the inner canvas's tool bar shows");
    let inner = app.place(BAR);
    assert!(
        (inner.0 - outer.0).abs() < 2.0 && inner.3 > inner.2,
        "it stands where the outer canvas kept its bar, at {inner:?} for {outer:?}"
    );
    app.snapshot("a_canvas_in_a_canvas_shows_its_tool_bar_in_place_of_the_outer_ones");

    let outside = format!("{:.0},{:.0}", card.0 + card.2 + 40.0, card.1 + card.3 / 2.0);
    app.run(&["click", &outside]);
    app.run(&["settle"]);
    assert_eq!(app.count(GRIP), 1, "letting go brings the outer bar back");
    assert_eq!(app.place(BAR), outer, "where it was");
    app.snapshot("a_canvas_in_a_canvas_gives_the_outer_tool_bar_back_when_let_go");
    app.close();
}
