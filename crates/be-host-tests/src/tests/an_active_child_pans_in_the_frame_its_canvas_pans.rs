use super::*;

const ARTWORK: &str = "#pan_zoom.card.4";

#[test]
fn an_active_child_pans_in_the_frame_its_canvas_pans() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Pan and Zoom");
    let card = host.place("GenericContainer #infinite-canvas.entity.");
    let inside = format!("{:.0},{:.0}", card.0 + card.2 / 2.0, card.1 + card.3 * 0.6);
    host.run(&["click", &inside]);
    host.wait("Pane \"Pan and Zoom\"");
    host.gone("#infinite-canvas.dock");
    host.run(&["settle"]);

    for _ in 0..3 {
        let before = host.place(ARTWORK);
        host.run(&["--no-settle", "wheel", &inside, "1"]);
        let first = host.place(ARTWORK);
        host.run(&["settle"]);
        let settled = host.place(ARTWORK);
        assert!(
            settled.1 < before.1,
            "the wheel pans the canvas under the editor, from {before:?} to {settled:?}"
        );
        assert_eq!(
            first, settled,
            "the editor is where the pan leaves it in the frame that took the wheel"
        );
    }
    host.close();
}
