use super::*;

const TOOL_BAR: &str = "GenericContainer #infinite-canvas.dock at";
const ZOOM_BAR: &str = "GenericContainer #infinite-canvas.zoom-pill at";

#[test]
fn the_canvas_zoom_bar_floats_at_any_corner_and_stays_there() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.wait("#infinite-canvas.zoom-pill");
    app.run(&["settle"]);
    let pane = app.place("Pane \"Canvas\"");
    let right = pane.0 + pane.2;
    let bottom = pane.1 + pane.3;
    let zoom = app.place(ZOOM_BAR);
    assert!(
        right - (zoom.0 + zoom.2) < 16.0 && zoom.1 - pane.1 < 16.0,
        "the zoom bar starts at the top right, at {zoom:?} in {pane:?}"
    );

    drag_bar(
        &mut app,
        "#infinite-canvas.zoom.grip",
        (pane.0 + 90.0, bottom - 80.0),
    );
    let zoom = app.place(ZOOM_BAR);
    assert!(
        zoom.0 - pane.0 < 16.0 && bottom - (zoom.1 + zoom.3) < 16.0 && zoom.2 > zoom.3,
        "dropped near the bottom left, it floats over the bottom at its left, at {zoom:?}"
    );
    let tools = app.place(TOOL_BAR);
    assert!(
        tools.0 > zoom.0 + zoom.2 || tools.1 + tools.3 < zoom.1,
        "the tool bar floating over the middle of the bottom stays clear of it, at {tools:?}"
    );
    app.snapshot("the_canvas_zoom_bar_dragged_to_the_bottom_left_corner");

    drag_bar(
        &mut app,
        "#infinite-canvas.dock.grip",
        (pane.0 + 150.0, pane.1 + 60.0),
    );
    drag_bar(
        &mut app,
        "#infinite-canvas.zoom.grip",
        (right - 150.0, pane.1 + 60.0),
    );
    let tools = app.place(TOOL_BAR);
    let zoom = app.place(ZOOM_BAR);
    assert!(
        tools.0 - pane.0 < 16.0 && tools.1 - pane.1 < 16.0 && tools.2 > tools.3,
        "the tool bar floats over the top at its left, at {tools:?}"
    );
    assert!(
        right - (zoom.0 + zoom.2) < 16.0 && (zoom.1 - tools.1).abs() < 1.0,
        "and the zoom bar beside it on the same edge, at its right, at {zoom:?}"
    );
    app.snapshot("both_canvas_bars_float_over_the_top_at_either_end");

    app.click("Button \"Close tab\"");
    app.gone("#infinite-canvas.dock");
    app.click("TreeItem \"Canvas\"");
    app.wait("#infinite-canvas.zoom-pill");
    app.run(&["settle"]);
    assert_eq!(
        (app.place(TOOL_BAR), app.place(ZOOM_BAR)),
        (tools, zoom),
        "the canvas opened again keeps both bars where they were"
    );
    app.close();
}
