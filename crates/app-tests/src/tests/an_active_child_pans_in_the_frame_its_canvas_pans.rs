use super::*;

const ARTWORK: &str = "#pixel-art.artwork";

#[test]
fn an_active_child_pans_in_the_frame_its_canvas_pans() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    add_to_canvas(&mut app, "Pixel Art");
    let card = app.place("GenericContainer #infinite-canvas.entity.");
    let inside = format!("{:.0},{:.0}", card.0 + card.2 / 2.0, card.1 + card.3 * 0.6);
    app.run(&["app", "click", &inside]);
    app.wait("Pane \"Pixel Art\"");
    app.gone("#infinite-canvas.dock");
    app.run(&["settle"]);

    for _ in 0..3 {
        let before = app.place(ARTWORK);
        app.run(&["--no-settle", "app", "wheel", &inside, "1"]);
        let first = app.place(ARTWORK);
        app.run(&["settle"]);
        let settled = app.place(ARTWORK);
        assert!(
            settled.1 < before.1,
            "the wheel pans the canvas under the pixel art, from {before:?} to {settled:?}"
        );
        assert_eq!(
            first, settled,
            "the pixel art is where the pan leaves it in the frame that took the wheel"
        );
    }
    app.close();
}
