use super::*;

const ARTWORK: &str = "#pan_zoom.card.4";

#[test]
fn a_pan_and_zoom_editor_on_a_canvas_takes_the_frame_in_place_and_pans_with_it() {
    let mut app = AppTest::start();
    app.run(&["resize", "1280x900"]);
    add_to_my_files(&mut app, "Canvas");
    add_to_canvas(&mut app, "Pan and Zoom");
    let card = app.place("GenericContainer #infinite-canvas.entity.");
    let inside = format!("{:.0},{:.0}", card.0 + card.2 / 2.0, card.1 + card.3 * 0.6);

    app.run(&["click", &inside]);
    app.wait("Pane \"Pan and Zoom\"");
    app.gone("#infinite-canvas.dock");
    assert_eq!(
        app.count("Pane \"Pan and Zoom\""),
        1,
        "the editor is described once"
    );
    let artwork = app.place(ARTWORK);
    assert!(
        (artwork.0 - card.0).abs() < card.2 && (artwork.1 - card.1).abs() < card.3,
        "the world stays where the card was, at {artwork:?} rather than {card:?}"
    );
    app.snapshot("a_pan_and_zoom_editor_on_a_canvas_takes_the_frame_in_place");

    app.run(&["wheel", &inside, "2"]);
    let panned = app.place(ARTWORK);
    assert!(
        panned.1 < artwork.1,
        "the wheel pans the canvas under the editor, from {artwork:?} to {panned:?}"
    );
    app.run(&["keydown", "ctrl"]);
    app.run(&["wheel", &inside, "-2"]);
    app.run(&["keyup", "ctrl"]);
    let zoomed = app.place(ARTWORK);
    assert!(
        zoomed.2 > panned.2,
        "ctrl and the wheel zoom in, from {panned:?} to {zoomed:?}"
    );

    let card = format!(
        "{:.0},{:.0}",
        zoomed.0 + zoomed.2 / 2.0,
        zoomed.1 + zoomed.3 / 2.0
    );
    app.run(&["click", &card]);

    app.key("Escape");
    app.wait("#infinite-canvas.dock");
    app.close();
}
