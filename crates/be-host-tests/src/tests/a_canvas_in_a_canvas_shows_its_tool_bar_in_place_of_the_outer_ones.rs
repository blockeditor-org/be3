use super::*;

const SHAPE: &str = "GenericContainer #infinite-canvas.entity.";
const GRIP: &str = "#infinite-canvas.dock.grip";
const BAR: &str = "GenericContainer #infinite-canvas.dock at";

#[test]
fn a_canvas_in_a_canvas_shows_its_tool_bar_in_place_of_the_outer_ones() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Canvas");
    host.click(GRIP);
    host.click("#grip.dock.bar.beside.left");
    host.run(&["settle"]);
    let outer = host.place(BAR);
    let card = host.place(SHAPE);

    let inside = format!("{:.0},{:.0}", card.0 + card.2 / 2.0, card.1 + card.3 * 0.6);
    host.run(&["click", &inside]);
    host.run(&["settle"]);
    assert_eq!(
        host.count(GRIP),
        1,
        "only the inner canvas's tool bar shows"
    );
    let inner = host.place(BAR);
    assert!(
        (inner.0 - outer.0).abs() < 2.0 && inner.3 > inner.2,
        "it stands where the outer canvas kept its bar, at {inner:?} for {outer:?}"
    );
    host.snapshot("a_canvas_in_a_canvas_shows_its_tool_bar_in_place_of_the_outer_ones");

    let outside = format!("{:.0},{:.0}", card.0 + card.2 + 40.0, card.1 + card.3 / 2.0);
    host.run(&["click", &outside]);
    host.run(&["settle"]);
    assert_eq!(host.count(GRIP), 1, "letting go brings the outer bar back");
    assert_eq!(host.place(BAR), outer, "where it was");
    host.snapshot("a_canvas_in_a_canvas_gives_the_outer_tool_bar_back_when_let_go");
    host.close();
}
