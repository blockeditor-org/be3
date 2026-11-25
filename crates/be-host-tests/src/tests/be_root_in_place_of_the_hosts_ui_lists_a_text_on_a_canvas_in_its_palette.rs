use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_lists_a_text_on_a_canvas_in_its_palette() {
    // be-host's own UI puts a text on a canvas; started again with the root in place of its
    // UI, Ctrl+Shift+P with the text's view holding the keyboard opens the one palette the
    // root draws over every app, as be-host's does: the text's actions first, then the
    // canvas's around it, then the workspace's, each under its app's name. The chord or Escape
    // closes it. Choosing the canvas's Add a block gives the keyboard to the canvas's view and
    // runs it, which opens its picker.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Text");
    let text = host.place("#text.surface");
    let text = format!("{:.0},{:.0}", text.0 + text.2 / 2.0, text.1 + text.3 / 2.0);
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.run(&["click", &text]);
    host.run(&["settle"]);
    host.key("ctrl+shift+p");
    host.run(&["settle"]);
    host.type_text("details");
    host.run(&["settle"]);
    host.snapshot("be_root_lists_a_text_on_a_canvas_in_its_palette");
    // The chord closes it again, and Escape closes it too.
    host.key("ctrl+shift+p");
    host.run(&["settle"]);
    host.snapshot("be_root_closes_its_palette");
    host.key("ctrl+shift+p");
    host.run(&["settle"]);
    host.key("Escape");
    host.run(&["settle"]);
    host.snapshot("be_root_closes_its_palette");
    host.key("ctrl+shift+p");
    host.run(&["settle"]);
    host.type_text("add a block");
    host.run(&["settle"]);
    host.key("Enter");
    host.run(&["settle"]);
    host.snapshot("be_root_runs_an_action_its_palette_lists");
    host.close();
}
