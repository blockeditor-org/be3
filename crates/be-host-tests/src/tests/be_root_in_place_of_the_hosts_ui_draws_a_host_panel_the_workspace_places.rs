use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_draws_a_host_panel_the_workspace_places() {
    // With the root in place of be-host's UI, its menu shows one of the host's panels as
    // be-host's does: it asks the workspace, which places it as a tab, and the root draws the
    // panel itself in that tab's place.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    // The root's menu button, at the window's top right.
    host.run(&["click", "1078,20"]);
    host.run(&["settle"]);
    // Its Block Stack State row.
    host.run(&["click", "997,351"]);
    host.run(&["settle"]);
    host.snapshot("be_root_draws_a_host_panel_the_workspace_places");
    host.close();
}
