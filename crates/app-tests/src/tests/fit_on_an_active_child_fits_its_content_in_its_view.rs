use super::*;

const ARTWORK: &str = "#pixel-art.artwork";

#[test]
fn fit_on_an_active_child_fits_its_content_in_its_view() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    add_to_canvas(&mut app, "Pixel Art");
    let card = app.place("GenericContainer #infinite-canvas.entity.");
    let inside = format!("{:.0},{:.0}", card.0 + card.2 / 2.0, card.1 + card.3 * 0.6);
    app.run(&["click", &inside]);
    app.wait("Pane \"Pixel Art\"");
    app.gone("#infinite-canvas.dock");
    app.run(&["wheel", &inside, "3"]);
    let panned = app.place(ARTWORK);
    app.click("#pixel-art.fit");
    let fitted = app.place(ARTWORK);
    let canvas = app.place("#pixel-art.canvas");
    let centre = |place: (f32, f32, f32, f32)| (place.0 + place.2 / 2.0, place.1 + place.3 / 2.0);
    let (fitted_centre, canvas_centre) = (centre(fitted), centre(canvas));
    assert!(
        (fitted_centre.0 - canvas_centre.0).abs() < 2.0
            && (fitted_centre.1 - canvas_centre.1).abs() < 2.0,
        "fit centres the pixel art in its own content, {panned:?} to {fitted:?} in {canvas:?}"
    );
    let filled = (fitted.2 / canvas.2).max(fitted.3 / canvas.3);
    assert!(
        (filled - 1.0).abs() < 0.01,
        "and fills it, {fitted:?} in {canvas:?}"
    );
    app.close();
}
