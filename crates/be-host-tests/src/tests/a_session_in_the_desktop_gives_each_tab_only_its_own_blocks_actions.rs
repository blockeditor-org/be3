use super::*;

const FILES_BAR: &str = "#dock.joined:2/files.";
const EDITORS_BAR: &str = "#dock.joined:2/editors.";

#[test]
fn a_session_in_the_desktop_gives_each_tab_only_its_own_blocks_actions() {
    let mut app = AppTest::start_desktop();
    app.click("#desktop.apps");
    app.click("#desktop.apps.sessions.new");
    app.wait("Pane \"Files\"");
    add_to_my_files(&mut app, "Canvas");
    app.key("Enter");
    app.wait("Tab \"Canvas\" selected");
    app.run(&["settle"]);
    assert_eq!(
        app.count(FILES_BAR),
        0,
        "the Files bar has none of the session block's actions and no menu"
    );
    for action in ["undo", "redo", "share"] {
        let canvas = format!("{EDITORS_BAR}pin.editor.{action} ");
        assert_eq!(app.count(&canvas), 1, "the canvas bar has one {action}");
    }
    app.crop("#desktop.dock");
    app.snapshot("a_session_in_the_desktop_gives_each_tab_only_its_own_blocks_actions");
    app.click(&format!("{EDITORS_BAR}menu"));
    app.wait(&format!("{EDITORS_BAR}menu.canvas.add"));
    for item in ["rename", "palette", "details"] {
        let entry = format!("{EDITORS_BAR}menu.editor.{item} ");
        assert_eq!(app.count(&entry), 1, "the canvas menu has one {item}");
    }
    app.key("Escape");
    app.close();
}
