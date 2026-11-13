use super::*;

const STROKE: &str = "GenericContainer #infinite-canvas.entity.";

#[test]
fn a_second_finger_turns_a_pen_stroke_into_a_pinch() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    let stage = host.place("#infinite-canvas.canvas");
    let at = |x: f32, y: f32| format!("{:.0},{:.0}", stage.0 + stage.2 * x, stage.1 + stage.3 * y);
    host.click("#infinite-canvas.dock.tool.Pen");

    host.run(&["touch", "down", &format!("0={}", at(0.45, 0.55))]);
    host.run(&["touch", "move", &format!("0={}", at(0.52, 0.55))]);
    host.run(&["touch", "move", &format!("0={}", at(0.6, 0.55))]);
    host.run(&["touch", "up"]);
    host.wait(STROKE);
    assert_eq!(host.count(STROKE), 1, "one finger draws a stroke");
    let drawn = host.place(STROKE);

    host.run(&["touch", "down", &format!("0={}", at(0.4, 0.5))]);
    host.run(&["touch", "move", &format!("0={}", at(0.45, 0.5))]);
    host.run(&["touch", "down", &format!("1={}", at(0.6, 0.5))]);
    host.run(&[
        "touch",
        "move",
        &format!("0={}", at(0.3, 0.5)),
        &format!("1={}", at(0.75, 0.5)),
    ]);
    host.run(&["touch", "up"]);
    host.run(&["settle"]);
    assert_eq!(
        host.count(STROKE),
        1,
        "a stroke that became a pinch draws nothing"
    );
    let zoomed = host.place(STROKE);
    assert!(
        zoomed.2 > drawn.2 * 2.0,
        "the two fingers zoomed the canvas, {drawn:?} to {zoomed:?}"
    );
    host.close();
}
