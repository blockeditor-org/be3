use super::*;

#[test]
#[should_panic(
    expected = "an app stopped while this test waited: Workspace stopped: It was crashed"
)]
fn a_wait_names_an_app_that_stopped_instead_of_running_out() {
    let mut host = HostTest::start(Options::default());
    host.click("#file-tree.host-menu");
    host.click("#host.menu.panel.Apps");
    host.click("#debug.apps.crash.be3.workspace-ui");
    host.wait("#sessions.keep");
    // The files went with the shell, so this waits for what will not come back, and fails as
    // soon as the slow wait looks, not when the whole wait runs out.
    host.wait("#file-tree.host-menu");
}
