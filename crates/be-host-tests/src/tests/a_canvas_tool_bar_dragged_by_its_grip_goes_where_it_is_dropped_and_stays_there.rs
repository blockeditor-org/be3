use super::*;

const CANVAS_BAR: &str = "GenericContainer #infinite-canvas.dock at";
const GRIP: &str = "#infinite-canvas.dock.grip";

#[test]
fn a_canvas_tool_bar_dragged_by_its_grip_goes_where_it_is_dropped_and_stays_there() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.dock");
    host.run(&["settle"]);
    let pane = host.place("Pane \"Canvas\"");
    let docked = host.place(CANVAS_BAR);
    assert!(docked.2 > docked.3, "the bar starts along the bottom");

    drag_bar(&mut host, GRIP, (pane.0 + 4.0, pane.1 + pane.3 / 2.0));
    let left = host.place(CANVAS_BAR);
    assert!(
        left.3 > left.2 && (left.0 - pane.0).abs() < 12.0,
        "dropped on the left edge, the bar docks along it, upright, at {left:?} in {pane:?}"
    );
    host.snapshot("a_canvas_tool_bar_dragged_to_the_left_edge_docks_along_it");

    drag_bar(&mut host, GRIP, (pane.0 + pane.2 / 2.0, pane.1 + 80.0));
    let top = host.place(CANVAS_BAR);
    assert!(
        top.2 > top.3 && top.1 > pane.1 + 2.0 && top.1 < pane.1 + 40.0,
        "dropped a little in from the top, it floats over the top, at {top:?} in {pane:?}"
    );
    host.snapshot("a_canvas_tool_bar_dragged_near_the_top_floats_over_it");

    host.click("Button \"Close tab\"");
    host.gone("#infinite-canvas.dock");
    host.click("TreeItem \"Canvas\"");
    host.wait("#infinite-canvas.dock");
    host.run(&["settle"]);
    assert_eq!(
        host.place(CANVAS_BAR),
        top,
        "the canvas opened again shows its bar where it was left"
    );

    let files = host.place("Pane \"Files\"");
    let away = (files.0 + files.2 / 2.0, files.1 + files.3 / 2.0);
    drag_bar(&mut host, GRIP, away);
    let popped = host.place(CANVAS_BAR);
    assert!(
        popped.0 < pane.0,
        "dropped away from the canvas, it pops out where it is dropped, at {popped:?}"
    );
    host.snapshot("a_canvas_tool_bar_dragged_away_from_the_canvas_pops_out_there");

    drag_bar(
        &mut host,
        GRIP,
        (pane.0 + pane.2 / 2.0, pane.1 + pane.3 - 80.0),
    );
    let back = host.place(CANVAS_BAR);
    assert!(
        back.2 > back.3 && back.0 > pane.0 && back.1 + back.3 > pane.1 + pane.3 - 40.0,
        "dragged back from its window, it floats over the bottom again, at {back:?}"
    );
    host.close();
}
