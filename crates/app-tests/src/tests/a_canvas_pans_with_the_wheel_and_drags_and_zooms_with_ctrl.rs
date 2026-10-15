use super::*;

const SHAPE: &str = "GenericContainer #infinite-canvas.entity.";

#[test]
fn a_canvas_pans_with_the_wheel_and_drags_and_zooms_with_ctrl() {
    let mut app = AppTest::start();
    add_root_block(&mut app, "Canvas");
    app.click("#infinite-canvas.dock.tool.Rectangle");
    app.run(&["drag", "400,200", "500,300"]);
    app.wait(SHAPE);
    let start = app.place(SHAPE);

    app.run(&["wheel", "500,400", "3"]);
    let wheeled = app.place(SHAPE);
    assert!(
        wheeled.1 < start.1 && wheeled.0 == start.0,
        "the wheel pans down, from {start:?} to {wheeled:?}"
    );

    app.run(&["scroll", "500,400", "0", "40"]);
    let scrolled = app.place(SHAPE);
    assert!(
        scrolled.0 < wheeled.0,
        "a sideways scroll pans sideways, from {wheeled:?} to {scrolled:?}"
    );

    app.run(&["down", "500,450", "middle"]);
    app.run(&["move", "560,470"]);
    app.run(&["up", "560,470", "middle"]);
    let dragged = app.place(SHAPE);
    assert_eq!(
        (dragged.0 - scrolled.0, dragged.1 - scrolled.1),
        (60.0, 20.0),
        "a middle drag carries the canvas with it"
    );

    app.run(&["keydown", "space"]);
    app.run(&["drag", "500,450", "440,450"]);
    app.run(&["keyup", "space"]);
    let spaced = app.place(SHAPE);
    assert_eq!(
        spaced.0 - dragged.0,
        -60.0,
        "a drag with space held pans rather than draws"
    );
    assert_eq!(app.count(SHAPE), 1, "the drags drew nothing");

    app.run(&["keydown", "ctrl"]);
    app.run(&["wheel", "500,400", "-2"]);
    app.run(&["keyup", "ctrl"]);
    let zoomed = app.place(SHAPE);
    assert!(
        zoomed.2 > spaced.2,
        "ctrl and the wheel zoom in, from {spaced:?} to {zoomed:?}"
    );
    app.close();
}
