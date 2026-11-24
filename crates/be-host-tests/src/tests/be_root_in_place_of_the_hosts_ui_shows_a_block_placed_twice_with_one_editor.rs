use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

const PANE: &str = "Pane \"Checklist\"";

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_a_block_placed_twice_with_one_editor() {
    // be-host's own UI puts a checklist on a canvas and duplicates its card, which places the
    // same block twice; started again with the root in place of its UI, the root shows both
    // places with one editor, as be-host does: the first place shows its view, the second a
    // copy of it.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    host.run(&["resize", "1700x900"]);
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Checklist");
    host.wait(PANE);
    host.wait("#checklist.add");
    let canvas = host.place("#infinite-canvas.canvas");
    let empty = format!("{:.0},{:.0}", canvas.0 + 20.0, canvas.1 + canvas.3 - 20.0);
    let card = host.place("GenericContainer #infinite-canvas.entity.");
    host.click(&empty);
    host.key("ctrl+a");
    host.key("ctrl+d");
    host.run(&["settle"]);
    // The duplicate is placed over the card, 24 points down and right: drag it aside by its
    // title bar, which is the canvas's.
    let title = (card.0 + card.2 / 2.0 + 24.0, card.1 + 24.0 + 25.0);
    let from = format!("{:.0},{:.0}", title.0, title.1);
    let to = format!("{:.0},{:.0}", title.0 + 470.0, title.1 + 40.0);
    host.run(&["drag", &from, &to]);
    host.run(&["settle"]);
    host.restart_as_root(ROOT);
    host.run(&["resize", "1700x900"]);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_a_block_placed_twice_with_one_editor");
    host.close();
}
