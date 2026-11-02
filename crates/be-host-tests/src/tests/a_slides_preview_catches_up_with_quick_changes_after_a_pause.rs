use super::*;

const SHAPE: &str = "GenericContainer #infinite-canvas.entity.";

#[test]
fn a_slides_preview_catches_up_with_quick_changes_after_a_pause() {
    let mut host = HostTest::start_drawing_previews();
    add_to_my_files(&mut host, "Presentation");
    host.click("#presentation.add");
    host.wait("#picker.slides");
    host.click("MenuItem \"Blank page\"");
    host.gone("#picker.slides");
    host.wait("Slide 1 of 1");
    host.wait("#infinite-canvas.canvas");
    let stage = host.place("#infinite-canvas.canvas");
    let at = |x: f32, y: f32| format!("{:.0},{:.0}", stage.0 + stage.2 * x, stage.1 + stage.3 * y);
    host.click("#infinite-canvas.dock.tool.Rectangle");
    host.run(&["drag", &at(0.2, 0.3), &at(0.35, 0.45)]);
    host.wait(SHAPE);
    let drawn = host.count(SHAPE);
    host.pause(600);
    host.key("ctrl+d");
    host.key("ctrl+d");
    assert_eq!(
        host.count(SHAPE),
        drawn + 2,
        "the rectangle was copied twice"
    );
    host.crop("#presentation.filmstrip");
    host.record();
    host.pause(600);
    host.snapshot("a_slides_preview_catches_up_with_quick_changes_after_a_pause");
    host.close();
}
