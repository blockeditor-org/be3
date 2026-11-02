use super::*;

const APPS: &str = "#desktop.apps.panel";

#[test]
fn super_f_with_no_window_focused_leaves_the_apps_panel_closed() {
    let mut host = HostTest::start_desktop();
    host.click("#desktop.status");
    host.key("Escape");
    host.run(&["settle"]);
    host.key("super+f");
    host.run(&["settle"]);
    assert_eq!(
        host.count(APPS),
        0,
        "Super held for an F the desktop's own fullscreen action heard is a chord, not a tap"
    );
    host.key("super");
    host.wait(APPS);
    host.key("Escape");
    host.close();
}
