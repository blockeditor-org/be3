use super::*;

const FILES_BAR: &str = "#dock.joined:2/files.";
const EDITORS_BAR: &str = "#dock.joined:2/editors.";

#[test]
fn a_session_in_the_desktop_gives_each_tab_only_its_own_blocks_actions() {
    let mut host = HostTest::start(Options {
        desktop: true,
        ..Options::default()
    });
    host.click("#desktop.apps");
    host.click("#desktop.apps.sessions.new");
    host.wait("Pane \"Files\"");
    add_to_my_files(&mut host, "Canvas");
    host.key("Enter");
    host.wait("Tab \"Canvas\" selected");
    host.run(&["settle"]);
    assert_eq!(
        host.count(FILES_BAR),
        0,
        "the Files bar has none of the session block's actions and no menu"
    );
    for action in ["undo", "redo", "share"] {
        let canvas = format!("{EDITORS_BAR}pin.editor.{action} ");
        assert_eq!(host.count(&canvas), 1, "the canvas bar has one {action}");
    }
    host.crop("#desktop.dock");
    host.snapshot("a_session_in_the_desktop_gives_each_tab_only_its_own_blocks_actions");
    host.click(&format!("{EDITORS_BAR}menu"));
    host.wait(&format!("{EDITORS_BAR}menu.canvas.add"));
    for item in ["rename", "palette", "details"] {
        let entry = format!("{EDITORS_BAR}menu.editor.{item} ");
        assert_eq!(host.count(&entry), 1, "the canvas menu has one {item}");
    }
    host.key("Escape");
    host.close();
}
