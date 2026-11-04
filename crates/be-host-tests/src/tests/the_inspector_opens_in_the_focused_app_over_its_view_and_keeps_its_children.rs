use super::*;

#[test]
fn the_inspector_opens_in_the_focused_app_over_its_view_and_keeps_its_children() {
    let mut host = HostTest::start();
    host.wait("Pane \"Files\"");
    host.click("Heading \"No file open\"");
    host.key("ctrl+shift+i");
    assert!(
        !host.shown("#inspector.dock"),
        "the host's own inspector stays shut while an app has the keyboard"
    );
    assert!(
        host.shown("TreeItem \"My files\""),
        "the file tree the workspace shows stays where it was:\n{}",
        host.tree()
    );
    host.snapshot("the_inspector_opens_in_the_focused_app_over_its_view_and_keeps_its_children");
    host.key("ctrl+shift+i");
    host.close();
}
