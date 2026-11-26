use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

const PANE: &str = "Pane \"Checklist\"";

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_a_checklist_in_place_on_a_canvas() {
    // be-host's own UI puts a checklist on a canvas, which places it in place with the size its
    // content takes; started again with the root in place of its UI, the root lays the
    // checklist out at that size (`host.assign-size`) and draws it at its place's zoom, which
    // is 1 until the root's canvas has a camera to zoom with (4g7d).
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Checklist");
    host.wait(PANE);
    host.wait("#checklist.add");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_a_checklist_in_place_on_a_canvas");
    host.close();
}
