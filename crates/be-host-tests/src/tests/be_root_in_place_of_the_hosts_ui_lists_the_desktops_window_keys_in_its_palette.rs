use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_lists_the_desktops_window_keys_in_its_palette() {
    // Started with the root in place of be-host's UI as a desktop, a session the Apps panel
    // opens is a window of the desktop's; with the keyboard in it, the root's palette lists the
    // desktop's window actions with their keys after the session's own, as
    // `the_host_palette_lists_the_desktops_window_keys` does under be-host's.
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.click("#desktop.apps");
    host.run(&["settle"]);
    host.click("#desktop.apps.sessions.new");
    host.wait("#workspace.files");
    host.run(&["settle"]);
    // Under be-host the window opened takes the keyboard; under the root it takes none yet
    // (plans/host_as_platform_adapter.md, 4g7g2), so a press gives it to the session's files.
    host.click("#file-tree.filter");
    host.run(&["settle"]);
    host.key("ctrl+shift+p");
    host.wait("#root.palette.search");
    host.type_text("window");
    host.run(&["settle"]);
    let tree = host.tree();
    for (label, keys) in [
        ("Close window", "Alt+F4"),
        ("Snap window to the left half", "Super+Left"),
        ("Snap window to the right half", "Super+Right"),
        ("Maximize window", "Super+Up"),
    ] {
        assert!(
            tree.contains(&format!("ListBoxOption \"{label}\"")),
            "the palette lists {label}: {tree}"
        );
        assert!(tree.contains(keys), "with its key, {keys}: {tree}");
    }
    host.snapshot("be_root_lists_the_desktops_window_keys_in_its_palette");
    host.key("Escape");
    host.close();
}
