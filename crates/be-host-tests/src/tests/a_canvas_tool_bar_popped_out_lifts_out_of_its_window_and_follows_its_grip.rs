use super::*;

const CANVAS_BAR: &str = "GenericContainer #infinite-canvas.dock at";
const GRIP: &str = "#infinite-canvas.dock.grip";

#[test]
fn a_canvas_tool_bar_popped_out_lifts_out_of_its_window_and_follows_its_grip() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.wait("#infinite-canvas.dock");
    app.run(&["settle"]);
    let pane = app.place("Pane \"Canvas\"");
    let files = app.place("Pane \"Files\"");
    drag_bar(
        &mut app,
        GRIP,
        (files.0 + files.2 / 2.0, files.1 + files.3 / 2.0),
    );
    let popped = app.place(CANVAS_BAR);
    assert!(
        popped.0 < pane.0,
        "dropped over the files, the bar pops out into a window there, at {popped:?}"
    );

    let grip = app.place(GRIP);
    let start = (grip.0 + grip.2 / 2.0, grip.1 + grip.3 / 2.0);
    let to = (pane.0 + pane.2 / 2.0, pane.1 + pane.3 / 2.0);
    app.run(&["down", GRIP]);
    for step in 1..=4 {
        let share = step as f32 / 4.0;
        let x = start.0 + (to.0 - start.0) * share;
        let y = start.1 + (to.1 - start.1) * share;
        app.run(&["move", &format!("{x},{y}")]);
    }
    app.run(&["pause", "200"]);
    let carried = app.place(CANVAS_BAR);
    let under = |rect: (f32, f32, f32, f32)| {
        to.0 >= rect.0 - 2.0
            && to.0 <= rect.0 + rect.2 + 2.0
            && to.1 >= rect.1 - 2.0
            && to.1 <= rect.1 + rect.3 + 2.0
    };
    assert!(
        under(carried),
        "carried by its grip, the bar leaves its window and follows the pointer over the canvas, \
         at {carried:?} for the pointer at {to:?}"
    );
    app.snapshot("a_popped_out_canvas_tool_bar_carried_over_the_canvas");

    app.run(&["up"]);
    app.run(&["pause", "1000"]);
    let landed = app.place(CANVAS_BAR);
    let inside = landed.0 >= pane.0
        && landed.1 >= pane.1
        && landed.0 + landed.2 <= pane.0 + pane.2
        && landed.1 + landed.3 <= pane.1 + pane.3;
    let gap = (landed.0 - pane.0)
        .min(landed.1 - pane.1)
        .min(pane.0 + pane.2 - landed.0 - landed.2)
        .min(pane.1 + pane.3 - landed.1 - landed.3);
    assert!(
        inside && gap < 40.0,
        "let go at rest over the canvas, the bar is dropped and springs to float over its nearest \
         edge, at {landed:?} in {pane:?}"
    );
    app.close();
}
