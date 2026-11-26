use super::*;

#[test]
fn ctrl_tab_walks_the_workspace_tabs_while_an_editor_has_the_keyboard() {
    let mut host = HostTest::start(Options::default());
    for name in ["One", "Two"] {
        start_adding_to_my_files(&mut host, "Text");
        host.key("ctrl+a");
        host.type_text(name);
        host.key("Enter");
        host.wait(&format!("TreeItem \"{name}\""));
    }
    host.wait("Tab \"Two\" selected");
    host.click("#text.surface");
    host.type_text("typed");
    host.wait("typed");
    // The text editor's view has the keyboard: the workspace takes Ctrl+Tab ahead of it.
    host.key("ctrl+tab");
    host.wait("Tab \"One\" selected");
    host.key("ctrl+shift+tab");
    host.wait("Tab \"Two\" selected");
    host.close();
}
