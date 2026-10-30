use super::*;

const TOOLBAR: &str = "GenericContainer #presentation.toolbar at";
const CANVAS_BAR: &str = "GenericContainer #infinite-canvas.dock at";

#[test]
fn a_slides_canvas_bar_shares_the_top_band_with_the_presentation_toolbar() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Presentation");
    app.click("#presentation.add");
    app.wait("#picker.slides");
    app.click("MenuItem \"Title page\"");
    app.gone("#picker.slides");
    app.wait("#infinite-canvas.dock");
    app.run(&["settle"]);
    assert_eq!(
        app.count("#presentation.toolbar "),
        1,
        "the deck keeps its toolbar"
    );
    assert_eq!(
        app.count("#infinite-canvas.dock "),
        1,
        "the slide's canvas joins with its bar"
    );
    assert!(
        app.shown("#presentation.filmstrip"),
        "and the filmstrip stays"
    );

    app.run(&["resize", "1400x720"]);
    app.click("#infinite-canvas.dock.grip");
    app.click("#grip.dock.bar.beside.top");
    app.run(&["settle"]);
    let toolbar = app.place(TOOLBAR);
    let tools = app.place(CANVAS_BAR);
    assert!(
        (tools.1 - toolbar.1).abs() < 1.0,
        "both stand in the one band along the top edge, at {toolbar:?} and {tools:?}"
    );
    assert!(
        toolbar.0 + toolbar.2 <= tools.0 + 1.0,
        "the deck's toolbar, the outer editor's, comes first along it: {toolbar:?} then {tools:?}"
    );
    app.snapshot("a_slides_canvas_bar_shares_the_top_band_with_the_presentation_toolbar");
    app.close();
}
