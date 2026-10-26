use super::*;

const ARTWORK: &str = "#pixel-art.artwork";

#[test]
fn a_pixel_art_on_a_canvas_takes_the_frame_in_place_and_pans_with_it() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    add_to_canvas(&mut app, "Pixel Art");
    let card = app.place("GenericContainer #infinite-canvas.entity.");
    let inside = format!("{:.0},{:.0}", card.0 + card.2 / 2.0, card.1 + card.3 * 0.6);

    app.run(&["app", "click", &inside]);
    app.wait("Pane \"Pixel Art\"");
    app.gone("#infinite-canvas.dock");
    assert_eq!(
        app.count("Pane \"Pixel Art\""),
        1,
        "the pixel art is described once"
    );
    let artwork = app.place(ARTWORK);
    assert!(
        (artwork.0 - card.0).abs() < card.2 && (artwork.1 - card.1).abs() < card.3,
        "the artwork stays where the card was, at {artwork:?} rather than {card:?}"
    );
    app.snapshot("a_pixel_art_on_a_canvas_takes_the_frame_in_place");

    app.run(&["app", "wheel", &inside, "2"]);
    let panned = app.place(ARTWORK);
    assert!(
        panned.1 < artwork.1,
        "the wheel pans the canvas under the pixel art, from {artwork:?} to {panned:?}"
    );
    app.run(&["app", "keydown", "ctrl"]);
    app.run(&["app", "wheel", &inside, "-2"]);
    app.run(&["app", "keyup", "ctrl"]);
    let zoomed = app.place(ARTWORK);
    assert!(
        zoomed.2 > panned.2,
        "ctrl and the wheel zoom in, from {panned:?} to {zoomed:?}"
    );

    let pixel = format!(
        "{:.0},{:.0}",
        zoomed.0 + zoomed.2 / 2.0,
        zoomed.1 + zoomed.3 / 2.0
    );
    app.run(&["app", "click", &pixel]);

    app.key("Escape");
    app.wait("#infinite-canvas.dock");
    app.gone("Pane \"Pixel Art\"");
    app.close();
}
