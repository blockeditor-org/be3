use super::*;

#[test]
fn a_control_focused_in_the_desktops_apps_panel_reads_as_focused() {
    let mut host = HostTest::start_desktop();
    host.click("#desktop.apps");
    host.wait("#desktop.apps.sessions.new");
    host.run(&["a11y", "#desktop.apps.sessions.new", "focus"]);
    host.wait("Button \"New session\" focused #desktop.apps.sessions.new");
    host.close();
}
