use super::*;

const SHAPE: &str = "GenericContainer #infinite-canvas.entity.";

#[test]
fn a_slides_preview_catches_up_with_quick_changes_after_a_pause() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Presentation");
    app.click("#presentation.add");
    app.wait("#picker.slides");
    app.click("MenuItem \"Blank page\"");
    app.gone("#picker.slides");
    app.wait("Slide 1 of 1");
    app.wait("#infinite-canvas.canvas");
    let stage = app.place("#infinite-canvas.canvas");
    let at = |x: f32, y: f32| format!("{:.0},{:.0}", stage.0 + stage.2 * x, stage.1 + stage.3 * y);
    app.click("#infinite-canvas.dock.tool.Rectangle");
    app.run(&["drag", &at(0.2, 0.3), &at(0.35, 0.45)]);
    app.wait(SHAPE);
    let drawn = app.count(SHAPE);
    app.pause(600);
    app.key("ctrl+d");
    app.key("ctrl+d");
    assert_eq!(
        app.count(SHAPE),
        drawn + 2,
        "the rectangle was copied twice"
    );
    app.crop("#workspace.block");
    app.record();
    app.pause(600);
    app.snapshot("a_slides_preview_catches_up_with_quick_changes_after_a_pause");
    app.close();
}
