use super::*;

const TOOLBAR: &str = "GenericContainer #presentation.toolbar at";
const CANVAS_BAR: &str = "GenericContainer #infinite-canvas.dock at";

#[test]
fn a_slides_canvas_bar_shares_the_top_band_with_the_presentation_toolbar() {
    let mut host = HostTest::start(Options { previews: true, ..Options::default() });
    add_to_my_files(&mut host, "Presentation");
    host.click("#presentation.add");
    host.wait("#picker.slides");
    host.click("MenuItem \"Title page\"");
    host.gone("#picker.slides");
    host.wait("#infinite-canvas.dock");
    host.run(&["settle"]);
    assert_eq!(
        host.count("#presentation.toolbar "),
        1,
        "the deck keeps its toolbar"
    );
    assert_eq!(
        host.count("#infinite-canvas.dock "),
        1,
        "the slide's canvas joins with its bar"
    );
    assert!(
        host.shown("#presentation.filmstrip"),
        "and the filmstrip stays"
    );

    host.run(&["resize", "1400x720"]);
    host.click("#infinite-canvas.dock.grip");
    host.click("#grip.dock.bar.beside.top");
    host.run(&["settle"]);
    let toolbar = host.place(TOOLBAR);
    let tools = host.place(CANVAS_BAR);
    assert!(
        (tools.1 - toolbar.1).abs() < 1.0,
        "both stand in the one band along the top edge, at {toolbar:?} and {tools:?}"
    );
    assert!(
        toolbar.0 + toolbar.2 <= tools.0 + 1.0,
        "the deck's toolbar, the outer editor's, comes first along it: {toolbar:?} then {tools:?}"
    );
    host.snapshot("a_slides_canvas_bar_shares_the_top_band_with_the_presentation_toolbar");
    host.close();
}
