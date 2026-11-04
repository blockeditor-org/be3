use super::*;

#[test]
fn the_inspector_chord_in_the_more_blocks_search_inspects_the_workspace() {
    let mut host = HostTest::start();
    host.click("Label value=\"My files\"");
    host.click("#file-tree.new");
    host.click("#picker.more");
    host.wait("TextInput \"Search\" focused");
    host.run(&["keydown", "ctrl"]);
    host.run(&["keydown", "shift"]);
    host.run(&["keydown", "i"]);
    host.run(&["keyup", "i"]);
    host.run(&["keyup", "shift"]);
    host.run(&["keyup", "ctrl"]);
    assert!(
        !host.shown("#inspector.dock"),
        "the chord opens the inspector of the app that has the keyboard, not the host's"
    );
    host.wait("TextInput \"Search\" focused");
    host.key("ctrl+shift+i");
    host.key("Escape");
    host.gone("#picker.search");
    host.close();
}
