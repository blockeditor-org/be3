use super::*;

#[test]
fn ctrl_tab_leaves_the_workspace_tabs_alone_while_the_host_palette_is_open() {
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
    host.key("ctrl+shift+p");
    host.wait("#host.palette.search");
    // The palette is be-host's own modal: the workspace's intercepted Ctrl+Tab stops at it.
    host.key("ctrl+tab");
    host.run(&["settle"]);
    host.key("Escape");
    host.gone("#host.palette.search");
    assert!(
        host.shown("Tab \"Two\" selected"),
        "the tab under the palette stays as it was"
    );
    host.click("#text.surface");
    host.key("ctrl+tab");
    host.wait("Tab \"One\" selected");
    host.close();
}
