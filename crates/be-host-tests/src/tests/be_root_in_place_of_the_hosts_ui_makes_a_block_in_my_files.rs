use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

const NAME: &str = "TextInput \"Name\" value=\"Canvas\"";

#[test]
fn be_root_in_place_of_the_hosts_ui_makes_a_block_in_my_files() {
    // With the root in place of be-host's UI, the files' New menu makes a block in My files as
    // be-host's does: its picker's Canvas makes a canvas, which the picker places in My files
    // (a `place` command, which the host does for the root's editors as for its own), and the
    // files open it in a tab and offer to name it in its row.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.click(NEW);
    host.click(NEW_IN_MY_FILES);
    host.wait("#picker.more");
    host.click("MenuItem \"Canvas\"");
    // The root names the canvas's pane after its app, as be-host does.
    host.wait("Pane \"Canvas\"");
    host.wait("#infinite-canvas.dock.block");
    host.wait(NAME);
    host.key("Enter");
    host.gone(NAME);
    host.wait("TreeItem \"Canvas\"");
    host.gone("TreeItem \"Nothing here yet\"");
    host.run(&["settle"]);
    host.snapshot("be_root_makes_a_block_in_my_files");
    host.close();
}
