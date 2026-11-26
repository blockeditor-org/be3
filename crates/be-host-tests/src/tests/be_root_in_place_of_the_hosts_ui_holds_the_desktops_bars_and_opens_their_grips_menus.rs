use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

const APPS_GRIP: &str = "#dock.bar.4398046511104.grip";
const WINDOWS_GRIP: &str = "#dock.bar.4398046511105.grip";

fn chosen(host: &mut HostTest, id: &str) -> bool {
    host.run(&["tree"])
        .lines()
        .any(|line| line.contains(&format!("#{id} ")) && line.contains("toggled=True"))
}

/// How many scroll views in drive's `tree` hold the node with `id`: its ancestors, each listed
/// above it indented less than everything between them. Where the host's tree lists the
/// desktop's bars and the menu depends on whose UI draws it, so only these count.
fn enclosing_scrolls(tree: &str, id: &str) -> usize {
    let indent = |line: &str| line.len() - line.trim_start().len();
    let lines: Vec<&str> = tree.lines().collect();
    let Some(at) = lines.iter().position(|line| line.contains(id)) else {
        return 0;
    };
    let mut depth = indent(lines[at]);
    let mut scrolls = 0;
    for line in lines[..at].iter().rev() {
        if indent(line) < depth {
            depth = indent(line);
            if line.trim_start().starts_with("ScrollView") {
                scrolls += 1;
            }
        }
    }
    scrolls
}

/// What `the_desktops_bars_hold_their_place_empty_and_their_grips_open_their_menu` does, its
/// paintings named after `painted`.
fn hold_the_bars(host: &mut HostTest, painted: &str) {
    host.wait("#desktop.windows.none");
    let hint = host.place("#desktop.windows.none");
    let grip = host.place(WINDOWS_GRIP);
    assert!(
        hint.2 > 4.0 * grip.2,
        "with no windows open the Windows bar keeps a chip's length and says so: {hint:?} beside its grip {grip:?}"
    );
    host.snapshot(&format!("{painted}_hold_their_place_empty"));

    host.run(&["click", WINDOWS_GRIP, "right"]);
    host.wait("#grip.dock.bar.beside.bottom");
    assert!(
        chosen(host, "grip.dock.bar.beside.bottom"),
        "a secondary click on a bar's grip opens its menu, its place checked"
    );
    host.key("Escape");
    host.gone("#grip.dock.bar.beside.bottom");

    host.click(APPS_GRIP);
    host.wait("#grip.dock.bar.beside.ends");
    assert!(
        chosen(host, "grip.dock.bar.beside.ends"),
        "a bar docked at an end checks the submenu that holds its place"
    );
    host.snapshot(&format!("{painted}_open_their_grips_menu_at_an_end"));
    host.key("Escape");
    host.gone("#grip.dock.bar.beside.ends");

    host.click("#desktop.apps");
    host.click("#desktop.apps.menu");
    host.wait("#host.menu.recovery-phrase");
    let tree = host.run(&["tree"]);
    assert!(
        tree.contains("#host.menu.inspector"),
        "every row of the host menu can be found by its id"
    );
    assert_eq!(
        enclosing_scrolls(&tree, "#host.menu.status"),
        1,
        "the host menu scrolls in one view: {tree}"
    );
}

#[test]
fn be_root_in_place_of_the_hosts_ui_holds_the_desktops_bars_and_opens_their_grips_menus() {
    // be-host's own UI shows the desktop's bars holding their place with nothing in them, their
    // grips opening their menus, and the host menu from the Apps panel
    // (`the_desktops_bars_hold_their_place_empty_and_their_grips_open_their_menu`). Started
    // again with the root in place of its UI, the root shows the same.
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    hold_the_bars(&mut host, "the_hosts_desktop_bars");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    hold_the_bars(&mut host, "be_roots_desktop_bars");
    host.close();
}
