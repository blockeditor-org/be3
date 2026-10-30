use super::*;

#[test]
fn the_inspector_chord_in_the_more_blocks_search_inspects_the_workspace() {
    let mut app = AppTest::start();
    app.click("Label value=\"My files\"");
    app.click("#file-tree.new");
    app.click("#picker.more");
    app.wait("TextInput \"Search\" focused");
    app.run(&["keydown", "ctrl"]);
    app.run(&["keydown", "shift"]);
    app.run(&["keydown", "i"]);
    app.run(&["keyup", "i"]);
    app.run(&["keyup", "shift"]);
    app.run(&["keyup", "ctrl"]);
    assert!(
        !app.shown("#inspector.dock"),
        "the chord opens the inspector of the plugin that has the keyboard, not the app's"
    );
    app.wait("TextInput \"Search\" focused");
    app.key("ctrl+shift+i");
    app.key("Escape");
    app.gone("#picker.search");
    app.close();
}
