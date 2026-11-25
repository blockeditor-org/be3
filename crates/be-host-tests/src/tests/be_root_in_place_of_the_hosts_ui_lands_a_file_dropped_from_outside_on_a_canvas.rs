use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// A PNG of one red pixel.
const PNG: [u8; 69] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xdd, 0x8d, 0xb0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
];

/// How many cards the canvas shows.
fn cards(host: &mut HostTest) -> usize {
    host.tree()
        .lines()
        .filter(|line| line.contains("#infinite-canvas.entity."))
        .count()
}

/// Drags the file at `path` over the window from outside it and drops it on the canvas: one card
/// more shows the image.
fn drop_on_canvas(host: &mut HostTest, path: &str) {
    let before = cards(host);
    let canvas = host.place("#infinite-canvas.canvas");
    let at = format!(
        "{:.0},{:.0}",
        canvas.0 + canvas.2 / 2.0,
        canvas.1 + canvas.3 / 2.0
    );
    host.run(&["grab", path]);
    host.run(&["drop", &at]);
    host.run(&["settle"]);
    let after = cards(host);
    assert_eq!(
        after,
        before + 1,
        "the dropped image lands on the canvas as a card of its own"
    );
}

#[test]
fn be_root_in_place_of_the_hosts_ui_lands_a_file_dropped_from_outside_on_a_canvas() {
    // An image dragged over the window from a file manager and dropped on a canvas lands on it,
    // under be-host's UI and, started again, under the root alike: the root takes files dragged
    // over its host's window (`input.drag-files`) to the view under the pointer that takes a
    // drag, as it carries a block, and the canvas, shown in a tab's body, hears them there.
    let file = std::env::temp_dir().join(format!("be3-dropped-{}.png", std::process::id()));
    std::fs::write(&file, PNG).expect("the test writes the image it drops");
    let path = file.to_string_lossy().into_owned();
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    host.run(&["settle"]);
    drop_on_canvas(&mut host, &path);
    host.restart_as_root(ROOT);
    host.wait("#infinite-canvas.canvas");
    host.run(&["settle"]);
    drop_on_canvas(&mut host, &path);
    host.snapshot("be_root_lands_a_file_dropped_from_outside_on_a_canvas");
    host.close();
    let _ = std::fs::remove_file(&file);
}
