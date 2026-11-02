use super::*;

const ARTWORK: &str = "#pan_zoom.card.4";

#[test]
fn fit_on_an_active_child_fits_its_content_in_its_view() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Pan and Zoom");
    let card = host.place("GenericContainer #infinite-canvas.entity.");
    let inside = format!("{:.0},{:.0}", card.0 + card.2 / 2.0, card.1 + card.3 * 0.6);
    host.run(&["click", &inside]);
    host.wait("Pane \"Pan and Zoom\"");
    host.gone("#infinite-canvas.dock");
    host.run(&["wheel", &inside, "3"]);
    let panned = host.place(ARTWORK);
    host.click("#pan_zoom.fit");
    let fitted = pan_zoom_world(&mut host);
    let canvas = host.place("#pan_zoom.stage");
    let centre = |place: (f32, f32, f32, f32)| (place.0 + place.2 / 2.0, place.1 + place.3 / 2.0);
    let (fitted_centre, canvas_centre) = (centre(fitted), centre(canvas));
    assert!(
        (fitted_centre.0 - canvas_centre.0).abs() < 3.0
            && (fitted_centre.1 - canvas_centre.1).abs() < 2.0,
        "fit centres the pan and zoom world in its own content, {panned:?} to {fitted:?} in {canvas:?}"
    );
    let filled = (fitted.2 / canvas.2).max(fitted.3 / canvas.3);
    assert!(
        (filled - 1.0).abs() < 0.02,
        "and fills it, {fitted:?} in {canvas:?}"
    );
    host.close();
}
