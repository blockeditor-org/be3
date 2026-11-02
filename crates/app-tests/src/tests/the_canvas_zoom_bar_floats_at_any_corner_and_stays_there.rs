use super::*;

const TOOL_BAR: &str = "GenericContainer #infinite-canvas.dock at";
const ZOOM_BAR: &str = "GenericContainer #infinite-canvas.zoom-pill at";
const MODE_BAR: &str = "GenericContainer #infinite-canvas.mode at";

#[test]
fn the_canvas_zoom_bar_floats_at_any_corner_and_stays_there() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.zoom-pill");
    host.run(&["settle"]);
    let pane = host.place("Pane \"Canvas\"");
    let right = pane.0 + pane.2;
    let bottom = pane.1 + pane.3;
    let zoom = host.place(ZOOM_BAR);
    assert!(
        right - (zoom.0 + zoom.2) < 16.0 && zoom.1 - pane.1 < 16.0,
        "the zoom bar starts at the top right, at {zoom:?} in {pane:?}"
    );

    drag_bar(
        &mut host,
        "#infinite-canvas.zoom.grip",
        (pane.0 + 90.0, bottom - 80.0),
    );
    let zoom = host.place(ZOOM_BAR);
    assert!(
        zoom.0 - pane.0 < 16.0 && bottom - (zoom.1 + zoom.3) < 16.0 && zoom.2 > zoom.3,
        "dropped near the bottom left, it floats over the bottom at its left, at {zoom:?}"
    );
    let tools = host.place(TOOL_BAR);
    assert!(
        tools.0 > zoom.0 + zoom.2 || tools.1 + tools.3 < zoom.1,
        "the tool bar floating over the middle of the bottom stays clear of it, at {tools:?}"
    );
    host.snapshot("the_canvas_zoom_bar_dragged_to_the_bottom_left_corner");

    drag_bar(
        &mut host,
        "#infinite-canvas.dock.grip",
        (pane.0 + 150.0, pane.1 + 60.0),
    );
    drag_bar(
        &mut host,
        "#infinite-canvas.zoom.grip",
        (right - 150.0, pane.1 + 60.0),
    );
    let tools = host.place(TOOL_BAR);
    let zoom = host.place(ZOOM_BAR);
    let mode = host.place(MODE_BAR);
    assert!(
        tools.0 - pane.0 < 16.0
            && mode.1 - pane.1 < 16.0
            && tools.1 >= mode.1 + mode.3
            && tools.1 - (mode.1 + mode.3) < 16.0
            && tools.2 > tools.3,
        "the tool bar floats over the top at its left, under the mode dock there, at {tools:?}"
    );
    assert!(
        right - (zoom.0 + zoom.2) < 16.0 && (zoom.1 - mode.1).abs() < 1.0,
        "and the zoom bar beside it on the same edge, at its right, at {zoom:?}"
    );
    host.snapshot("both_canvas_bars_float_over_the_top_at_either_end");

    host.click("Button \"Close tab\"");
    host.gone("#infinite-canvas.dock");
    host.click("TreeItem \"Canvas\"");
    host.wait("#infinite-canvas.zoom-pill");
    host.run(&["settle"]);
    assert_eq!(
        (host.place(TOOL_BAR), host.place(ZOOM_BAR)),
        (tools, zoom),
        "the canvas opened again keeps both bars where they were"
    );
    host.close();
}
