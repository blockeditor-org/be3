use super::*;

const SHAPE: &str = "GenericContainer #infinite-canvas.entity.";

#[test]
fn a_canvas_pans_with_the_wheel_and_drags_and_zooms_with_ctrl() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.wait("#infinite-canvas.canvas");
    let stage = app.place("#infinite-canvas.canvas");
    let at = |x: f32, y: f32| format!("{:.0},{:.0}", stage.0 + stage.2 * x, stage.1 + stage.3 * y);
    app.click("#infinite-canvas.dock.tool.Rectangle");
    app.run(&["app", "drag", &at(0.3, 0.3), &at(0.45, 0.45)]);
    app.wait(SHAPE);
    let start = app.place(SHAPE);
    let middle = at(0.5, 0.6);

    app.run(&["app", "wheel", &middle, "3"]);
    let wheeled = app.place(SHAPE);
    assert!(
        wheeled.1 < start.1 && wheeled.0 == start.0,
        "the wheel pans down, from {start:?} to {wheeled:?}"
    );

    app.run(&["app", "scroll", &middle, "0", "40"]);
    let scrolled = app.place(SHAPE);
    assert!(
        scrolled.0 < wheeled.0,
        "a sideways scroll pans sideways, from {wheeled:?} to {scrolled:?}"
    );

    app.run(&["app", "down", &at(0.5, 0.7), "middle"]);
    app.run(&["app", "move", &at(0.6, 0.75)]);
    app.run(&["app", "up", &at(0.6, 0.75), "middle"]);
    let dragged = app.place(SHAPE);
    assert!(
        dragged.0 > scrolled.0 && dragged.1 > scrolled.1,
        "a middle drag carries the canvas with it, from {scrolled:?} to {dragged:?}"
    );

    app.run(&["app", "keydown", "space"]);
    app.run(&["app", "drag", &at(0.5, 0.7), &at(0.4, 0.7)]);
    app.run(&["app", "keyup", "space"]);
    let spaced = app.place(SHAPE);
    assert!(
        spaced.0 < dragged.0,
        "a drag with space held pans rather than draws, from {dragged:?} to {spaced:?}"
    );
    assert_eq!(app.count(SHAPE), 1, "the drags drew nothing");

    app.run(&["app", "keydown", "ctrl"]);
    app.run(&["app", "wheel", &middle, "-2"]);
    app.run(&["app", "keyup", "ctrl"]);
    let zoomed = app.place(SHAPE);
    assert!(
        zoomed.2 > spaced.2,
        "ctrl and the wheel zoom in, from {spaced:?} to {zoomed:?}"
    );
    app.close();
}
