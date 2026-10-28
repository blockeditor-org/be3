use super::*;

const ARTWORK: &str = "#pixel-art.artwork";
const STAGE: &str = "#pixel-art.canvas";

type Place = (f32, f32, f32, f32);

fn centre(place: Place) -> (f32, f32) {
    (place.0 + place.2 / 2.0, place.1 + place.3 / 2.0)
}

fn resize(app: &mut AppTest, size: &str) -> (Place, Place) {
    let before = app.place(STAGE);
    app.run(&["--no-settle", "resize", size]);
    let mut first = None;
    for _ in 0..4 {
        let stage = app.place(STAGE);
        if stage.2 != before.2 || stage.3 != before.3 {
            first = Some(app.place(ARTWORK));
            break;
        }
        app.run(&["--no-settle", "pause", "0"]);
    }
    let first = first.unwrap_or_else(|| panic!("the editor never drew at {size}"));
    app.run(&["settle"]);
    let settled = app.place(ARTWORK);
    assert_eq!(
        first, settled,
        "the first frame the editor draws at {size} shows the artwork where it settles"
    );
    (settled, app.place(STAGE))
}

#[test]
fn a_tab_keeps_its_view_through_a_resize() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Pixel Art");
    app.wait(ARTWORK);
    app.gone("#editor.loading");
    app.run(&["settle"]);

    let (fitted, stage) = resize(&mut app, "1300x820");
    let (artwork, middle) = (centre(fitted), centre(stage));
    assert!(
        (artwork.0 - middle.0).abs() < 1.0 && (artwork.1 - middle.1).abs() < 1.0,
        "an artwork kept fitted stays centred in its stage, {fitted:?} in {stage:?}"
    );
    assert!(
        fitted.2 <= stage.2 + 1.0 && fitted.3 <= stage.3 + 1.0,
        "and stays inside it, {fitted:?} in {stage:?}"
    );
    app.snapshot("a_tab_keeps_its_view_through_a_resize_fitted");

    let off_centre = format!(
        "{:.0},{:.0}",
        stage.0 + stage.2 * 0.3,
        stage.1 + stage.3 * 0.4
    );
    app.run(&["keydown", "ctrl"]);
    app.run(&["wheel", &off_centre, "-2"]);
    app.run(&["keyup", "ctrl"]);
    app.run(&["settle"]);
    let zoomed = app.place(ARTWORK);
    let offset = |artwork: Place, stage: Place| {
        let (artwork, stage) = (centre(artwork), centre(stage));
        (artwork.0 - stage.0, artwork.1 - stage.1)
    };
    let before = offset(zoomed, stage);
    assert!(
        before.0.abs() > 10.0 || before.1.abs() > 10.0,
        "a zoom away from the middle pans the artwork off it, {zoomed:?} in {stage:?}"
    );

    let (moved, resized) = resize(&mut app, "1200x700");
    let after = offset(moved, resized);
    assert!(
        (after.0 - before.0).abs() < 1.0 && (after.1 - before.1).abs() < 1.0,
        "a camera the person moved keeps its pan from the middle of the stage through a resize, {before:?} to {after:?}"
    );
    app.run(&["move", "Pane \"Files\""]);
    app.snapshot("a_tab_keeps_its_view_through_a_resize_moved");
    app.close();
}
