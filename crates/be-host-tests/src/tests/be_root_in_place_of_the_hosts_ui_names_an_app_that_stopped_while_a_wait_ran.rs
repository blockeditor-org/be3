use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
#[should_panic(
    expected = "an app stopped while this test waited: Workspace stopped: It was crashed"
)]
fn be_root_in_place_of_the_hosts_ui_names_an_app_that_stopped_while_a_wait_ran() {
    // be-host says an app of its own that crashed stopped, in a toast a test's wait reads
    // (`a_wait_names_an_app_that_stopped_instead_of_running_out`). With the root in place of
    // its UI, the root says the same of an app it runs, crashed from its Running Apps.
    let mut host = HostTest::root_with_dev_workspace(ROOT);
    host.run(&["settle"]);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.panel.Apps");
    host.click("#debug.apps.crash.be3.workspace-ui");
    host.wait("#sessions.keep");
    // The files went with the session's app, so this waits for what will not come back, and
    // fails as soon as the slow wait looks, not when the whole wait runs out.
    host.wait("#file-tree.host-menu");
}
