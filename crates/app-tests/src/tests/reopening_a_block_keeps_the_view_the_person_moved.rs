use super::*;

const ARTWORK: &str = "#pixel_ray_tracer.artwork";

#[test]
fn reopening_a_block_keeps_the_view_the_person_moved() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Pixel Ray Tracer");
    app.wait(ARTWORK);
    app.gone("#editor.loading");
    app.run(&["settle"]);
    let fitted = app.place(ARTWORK);
    let stage = app.place("#pixel_ray_tracer.canvas");
    let middle = format!(
        "{:.0},{:.0}",
        stage.0 + stage.2 * 0.5,
        stage.1 + stage.3 * 0.5
    );

    app.run(&["keydown", "ctrl"]);
    app.run(&["wheel", &middle, "-2"]);
    app.run(&["keyup", "ctrl"]);
    app.run(&["settle"]);
    let zoomed = app.place(ARTWORK);
    assert!(
        zoomed.2 > fitted.2 * 1.05,
        "ctrl and the wheel zoom in, from {fitted:?} to {zoomed:?}"
    );

    app.click("Button \"Close tab\"");
    app.gone(ARTWORK);
    app.click("TreeItem \"Pixel Ray Tracer\"");
    app.wait(ARTWORK);
    app.gone("#editor.loading");
    app.run(&["settle"]);
    let reopened = app.place(ARTWORK);
    assert!(
        (reopened.2 - zoomed.2).abs() < 1.0 && (reopened.0 - zoomed.0).abs() < 1.0,
        "the reopened block keeps the zoom the person chose, {reopened:?} rather than {zoomed:?}"
    );
    app.close();
}
