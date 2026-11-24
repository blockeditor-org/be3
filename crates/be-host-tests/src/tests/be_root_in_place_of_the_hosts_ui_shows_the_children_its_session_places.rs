use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// The workspace's Settings tab, above its side panel, in the window's points.
const SETTINGS_TAB: &str = "145,19";

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_the_children_its_session_places() {
    // The workspace the root shows places the Files in its side panel as a child: the root
    // shows the block's editor there, in an app of its own the host serves, and the host tells
    // the workspace it is available, so the files list shows in place of "Files are loading…"
    // (be_root_signed_in shows it).
    let mut host = HostTest::root_with_dev_workspace(ROOT);
    host.run(&["settle"]);
    // A press on the workspace's own Settings tab reaches the workspace, which places the
    // settings in the side panel in place of the Files: the root lets go of the Files' editor
    // and shows the settings' there.
    host.click(SETTINGS_TAB);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_its_sessions_children_replaced");
    host.close();
}
