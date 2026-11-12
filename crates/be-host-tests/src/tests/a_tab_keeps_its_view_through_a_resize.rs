use super::*;

const ARTWORK: &str = "#pan_zoom.card.4";
const STAGE: &str = "#pan_zoom.stage";

type Place = (f32, f32, f32, f32);

fn centre(place: Place) -> (f32, f32) {
    (place.0 + place.2 / 2.0, place.1 + place.3 / 2.0)
}

fn resize(host: &mut HostTest, size: &str) -> (Place, Place) {
    let before = host.place(STAGE);
    host.run(&["--no-settle", "resize", size]);
    let mut first = None;
    for _ in 0..4 {
        let stage = host.place(STAGE);
        if stage.2 != before.2 || stage.3 != before.3 {
            first = Some(host.place(ARTWORK));
            break;
        }
        host.run(&["--no-settle", "pause", "0"]);
    }
    let first = first.unwrap_or_else(|| panic!("the editor never drew at {size}"));
    host.run(&["settle"]);
    let settled = host.place(ARTWORK);
    assert_eq!(
        first, settled,
        "the first frame the editor draws at {size} shows the cards where they settle"
    );
    (settled, host.place(STAGE))
}

#[test]
fn a_tab_keeps_its_view_through_a_resize() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Pan and Zoom");
    host.wait(ARTWORK);
    host.gone("#editor.loading");
    host.run(&["settle"]);

    let (_, stage) = resize(&mut host, "1300x600");
    let fitted = pan_zoom_world(&mut host);
    let (artwork, middle) = (centre(fitted), centre(stage));
    assert!(
        (artwork.0 - middle.0).abs() < 2.0 && (artwork.1 - middle.1).abs() < 2.0,
        "a world kept fitted stays centred in its stage, {fitted:?} in {stage:?}"
    );
    assert!(
        fitted.2 <= stage.2 + 2.0 && fitted.3 <= stage.3 + 2.0,
        "and stays inside it, {fitted:?} in {stage:?}"
    );
    host.snapshot("a_tab_keeps_its_view_through_a_resize_fitted");

    let off_centre = format!(
        "{:.0},{:.0}",
        stage.0 + stage.2 * 0.3,
        stage.1 + stage.3 * 0.4
    );
    host.run(&["keydown", "ctrl"]);
    host.run(&["wheel", &off_centre, "-2"]);
    host.run(&["keyup", "ctrl"]);
    host.run(&["settle"]);
    let zoomed = host.place(ARTWORK);
    let offset = |artwork: Place, stage: Place| {
        let (artwork, stage) = (centre(artwork), centre(stage));
        (artwork.0 - stage.0, artwork.1 - stage.1)
    };
    let before = offset(zoomed, stage);
    assert!(
        before.0.abs() > 10.0 || before.1.abs() > 10.0,
        "a zoom away from the middle pans the cards off it, {zoomed:?} in {stage:?}"
    );

    let (moved, resized) = resize(&mut host, "1200x560");
    let after = offset(moved, resized);
    assert!(
        (after.0 - before.0).abs() <= 1.0 && (after.1 - before.1).abs() <= 1.0,
        "a camera the person moved keeps its pan from the middle of the stage through a resize, {before:?} to {after:?}"
    );
    host.run(&["move", "Pane \"Files\""]);
    host.snapshot("a_tab_keeps_its_view_through_a_resize_moved");
    host.close();
}
