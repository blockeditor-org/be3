use super::*;

#[test]
fn the_host_palette_lists_the_desktops_window_keys() {
    let mut host = HostTest::start(Options {
        desktop: true,
        ..Options::default()
    });
    host.click("#desktop.apps");
    host.click("#desktop.apps.sessions.new");
    host.wait("Pane \"Files\"");
    host.run(&["settle"]);
    host.key("ctrl+shift+p");
    host.wait("#host.palette.search");
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
    host.snapshot("the_host_palette_lists_the_desktops_window_keys");
    host.key("Escape");
    host.close();
}
