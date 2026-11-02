use super::*;

#[test]
fn the_inspector_opens_in_the_focused_plugin_over_its_view_and_keeps_its_children() {
    let mut app = AppTest::start();
    app.wait("Pane \"Files\"");
    app.click("Heading \"No file open\"");
    app.key("ctrl+shift+i");
    assert!(
        !app.shown("#inspector.dock"),
        "the app's own inspector stays shut while a plugin has the keyboard"
    );
    assert!(
        app.shown("TreeItem \"My files\""),
        "the file tree the workspace shows stays where it was:\n{}",
        app.tree()
    );
    app.snapshot("the_inspector_opens_in_the_focused_plugin_over_its_view_and_keeps_its_children");
    app.key("ctrl+shift+i");
    app.close();
}
