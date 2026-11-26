use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_leaves_the_workspace_tabs_alone_while_its_palette_is_open() {
    // be-host's own UI makes two texts, One and Two; started again with the root in place of
    // its UI, Ctrl+Tab while the root's palette is open stops at the palette, the root's modal,
    // and the tab under it stays as it was; with the palette closed, Ctrl+Tab walks the
    // workspace's tabs, as `ctrl_tab_leaves_the_workspace_tabs_alone_while_the_host_palette_is_open`
    // does under be-host's.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    for name in ["One", "Two"] {
        start_adding_to_my_files(&mut host, "Text");
        host.key("ctrl+a");
        host.type_text(name);
        host.key("Enter");
        host.wait(&format!("TreeItem \"{name}\""));
    }
    host.wait("Tab \"Two\" selected");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.wait("Tab \"Two\" selected");
    host.click("#text.surface");
    host.key("ctrl+shift+p");
    host.wait("#root.palette.search");
    host.key("ctrl+tab");
    host.run(&["settle"]);
    host.key("Escape");
    host.gone("#root.palette.search");
    assert!(
        host.shown("Tab \"Two\" selected"),
        "the tab under the palette stays as it was"
    );
    host.click("#text.surface");
    host.key("ctrl+tab");
    host.wait("Tab \"One\" selected");
    host.close();
}
