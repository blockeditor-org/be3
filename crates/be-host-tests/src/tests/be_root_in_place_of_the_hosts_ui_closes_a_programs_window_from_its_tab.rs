use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// The close button of the tab showing the program's window: the dock's tab close of the one
/// tab the desktop opened.
fn tab_close(host: &mut HostTest) -> String {
    let tree = host.tree();
    let closes: Vec<&str> = tree
        .split_whitespace()
        .filter(|word| word.starts_with("#dock.tab.") && word.ends_with(".close"))
        .collect();
    match closes.as_slice() {
        [close] => (*close).to_owned(),
        _ => panic!("one tab shows the window, with one close button: {closes:?}\n{tree}"),
    }
}

#[test]
fn be_root_in_place_of_the_hosts_ui_closes_a_programs_window_from_its_tab() {
    // A press on the close button of a program window's tab asks the desktop host to close the
    // window, which its program does: it leaves the windows bar, under be-host's own UI and,
    // started again, under the root alike, which relays the close to its host (the root's
    // `tell-shell`). The window is named by its title in its tab and chip under both.
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    host.open_test_window("Test window");
    host.wait("Button \"Be tool wayland window — Test window\"");
    host.run(&["settle"]);
    let close = tab_close(&mut host);
    host.click(&close);
    host.wait(" #desktop.windows.none");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.open_test_window("Test window");
    host.wait("Button \"Be tool wayland window — Test window\"");
    host.run(&["settle"]);
    let close = tab_close(&mut host);
    host.click(&close);
    host.wait(" #desktop.windows.none");
    host.close();
}
