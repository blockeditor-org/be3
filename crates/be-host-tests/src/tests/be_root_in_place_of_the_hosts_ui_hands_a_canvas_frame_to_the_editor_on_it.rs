use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_hands_a_canvas_frame_to_the_editor_on_it() {
    // be-host's own UI makes a canvas with a pan and zoom editor on it; started again with the
    // root in place of its UI, a press in the editor's card focuses it, and the root shows it
    // taking the canvas's frame: its own bars in place of the canvas's, its content where the
    // card was.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    host.run(&["resize", "1280x900"]);
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Pan and Zoom");
    let card = host.place("GenericContainer #infinite-canvas.entity.");
    let inside = format!("{:.0},{:.0}", card.0 + card.2 / 2.0, card.1 + card.3 * 0.6);
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_an_editor_on_a_canvas");
    host.run(&["click", &inside]);
    host.run(&["settle"]);
    host.snapshot("be_root_hands_a_canvas_frame_to_the_editor_on_it");
    host.close();
}
