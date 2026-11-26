use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_makes_a_block_in_my_files() {
    // With the root in place of be-host's UI, the files' New menu makes a block in My files as
    // be-host's does: its picker's Canvas makes a canvas, which opens in a pane of its own for
    // its name, as crates/be-host/web-test's popover test does on the page.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.click("#file-tree.new");
    host.click("MenuItem \"New in My files…\"");
    host.wait("#picker.more");
    host.click("MenuItem \"Canvas\"");
    host.wait("Pane \"Canvas\"");
    host.close();
}
