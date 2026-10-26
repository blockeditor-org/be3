use super::*;

const STROKE: &str = "GenericContainer #infinite-canvas.entity.";

#[test]
fn a_second_finger_turns_a_pen_stroke_into_a_pinch() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.wait("#infinite-canvas.canvas");
    let stage = app.place("#infinite-canvas.canvas");
    let at = |x: f32, y: f32| format!("{:.0},{:.0}", stage.0 + stage.2 * x, stage.1 + stage.3 * y);
    app.click("#infinite-canvas.dock.tool.Pen");

    app.run(&["app", "touch", "down", &format!("0={}", at(0.45, 0.55))]);
    app.run(&["app", "touch", "move", &format!("0={}", at(0.52, 0.55))]);
    app.run(&["app", "touch", "move", &format!("0={}", at(0.6, 0.55))]);
    app.run(&["app", "touch", "up"]);
    app.wait(STROKE);
    assert_eq!(app.count(STROKE), 1, "one finger draws a stroke");
    let drawn = app.place(STROKE);

    app.run(&["app", "touch", "down", &format!("0={}", at(0.4, 0.5))]);
    app.run(&["app", "touch", "move", &format!("0={}", at(0.45, 0.5))]);
    app.run(&["app", "touch", "down", &format!("1={}", at(0.6, 0.5))]);
    app.run(&[
        "app", "touch",
        "move",
        &format!("0={}", at(0.3, 0.5)),
        &format!("1={}", at(0.75, 0.5)),
    ]);
    app.run(&["app", "touch", "up"]);
    app.run(&["settle"]);
    assert_eq!(
        app.count(STROKE),
        1,
        "a stroke that became a pinch draws nothing"
    );
    let zoomed = app.place(STROKE);
    assert!(
        zoomed.2 > drawn.2 * 2.0,
        "the two fingers zoomed the canvas, {drawn:?} to {zoomed:?}"
    );
    app.close();
}
