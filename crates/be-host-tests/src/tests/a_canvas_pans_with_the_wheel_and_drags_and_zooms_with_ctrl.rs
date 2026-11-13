use super::*;

const SHAPE: &str = "GenericContainer #infinite-canvas.entity.";

#[test]
fn a_canvas_pans_with_the_wheel_and_drags_and_zooms_with_ctrl() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    let stage = host.place("#infinite-canvas.canvas");
    let at = |x: f32, y: f32| format!("{:.0},{:.0}", stage.0 + stage.2 * x, stage.1 + stage.3 * y);
    host.click("#infinite-canvas.dock.tool.Rectangle");
    host.run(&["drag", &at(0.3, 0.3), &at(0.45, 0.45)]);
    host.wait(SHAPE);
    let start = host.place(SHAPE);
    let middle = at(0.5, 0.6);

    host.run(&["wheel", &middle, "3"]);
    let wheeled = host.place(SHAPE);
    assert!(
        wheeled.1 < start.1 && wheeled.0 == start.0,
        "the wheel pans down, from {start:?} to {wheeled:?}"
    );

    host.run(&["scroll", &middle, "0", "40"]);
    let scrolled = host.place(SHAPE);
    assert!(
        scrolled.0 < wheeled.0,
        "a sideways scroll pans sideways, from {wheeled:?} to {scrolled:?}"
    );

    host.run(&["down", &at(0.5, 0.7), "middle"]);
    host.run(&["move", &at(0.6, 0.75)]);
    host.run(&["up", &at(0.6, 0.75), "middle"]);
    let dragged = host.place(SHAPE);
    assert!(
        dragged.0 > scrolled.0 && dragged.1 > scrolled.1,
        "a middle drag carries the canvas with it, from {scrolled:?} to {dragged:?}"
    );

    host.run(&["keydown", "space"]);
    host.run(&["drag", &at(0.5, 0.7), &at(0.4, 0.7)]);
    host.run(&["keyup", "space"]);
    let spaced = host.place(SHAPE);
    assert!(
        spaced.0 < dragged.0,
        "a drag with space held pans rather than draws, from {dragged:?} to {spaced:?}"
    );
    assert_eq!(host.count(SHAPE), 1, "the drags drew nothing");

    host.run(&["keydown", "ctrl"]);
    host.run(&["wheel", &middle, "-2"]);
    host.run(&["keyup", "ctrl"]);
    let zoomed = host.place(SHAPE);
    assert!(
        zoomed.2 > spaced.2,
        "ctrl and the wheel zoom in, from {spaced:?} to {zoomed:?}"
    );
    host.close();
}
