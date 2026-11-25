use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// Opens a new session from the desktop's Apps panel, pressed by name.
fn new_session(host: &mut HostTest) {
    host.click("#desktop.apps");
    host.run(&["settle"]);
    host.click("#desktop.apps.sessions.new");
    host.run(&["settle"]);
}

#[test]
fn be_root_in_place_of_the_hosts_ui_floats_a_window_the_desktop_opens() {
    // The desktop's Apps panel opens a session in a window of its own, under be-host's UI and
    // under the root alike, pressed by name in both: a tab of the desktop's dock window, whose
    // workspace's dock joins the window's, its Files, Settings and History tabs and its
    // editors' area in it. Under the root it is the second session, since the first is kept,
    // and opens beside where the first did.
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    new_session(&mut host);
    host.snapshot("the_host_floats_a_window_the_desktop_opens");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    new_session(&mut host);
    host.snapshot("be_root_floats_a_window_the_desktop_opens");
    host.close();
}
