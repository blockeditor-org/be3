use super::*;

const STROKE: &str = "GenericContainer #infinite-canvas.entity.";

#[test]
fn a_pen_draws_a_line_on_a_canvas_with_the_pen_tool() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    let stage = host.place("#infinite-canvas.canvas");
    let at = |x: f32, y: f32| format!("{:.0},{:.0}", stage.0 + stage.2 * x, stage.1 + stage.3 * y);
    host.click("#infinite-canvas.dock.tool.Pen");

    host.run(&["pen", "draw", &at(0.2, 0.4), &at(0.7, 0.5), "12"]);
    host.wait(STROKE);
    assert_eq!(host.count(STROKE), 1, "the pen drew one stroke");
    let stroke = host.place(STROKE);
    assert!(
        stroke.2 > stage.2 * 0.4,
        "the stroke runs from where the pen touched down to where it lifted, {stroke:?} on {stage:?}"
    );
    host.run(&["pen", "leave"]);
    host.crop("#infinite-canvas.canvas");
    host.snapshot("a_pen_draws_a_line_on_a_canvas_with_the_pen_tool");
    host.close();
}
