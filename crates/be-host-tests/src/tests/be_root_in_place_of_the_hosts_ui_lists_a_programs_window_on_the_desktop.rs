use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_lists_a_programs_window_on_the_desktop() {
    // A program run in the desktop's compositor opens a window, which the desktop's windows bar
    // lists, under be-host's own UI and, started again, under the root alike: the root drives
    // the desktop host as be-host's UI does, and the window reaches the desktop app, the shell,
    // which opens it in a tab that shows it, composited by be-host where the root placed it.
    // Its tab and chip are named by its window's title, which reaches the desktop as its
    // child's status, under the root as under be-host.
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    host.open_test_window("Test window");
    // Titled once the program says its title. Settling waits for the window to draw at the
    // size its tab configured it to, so the whole window is painted.
    host.wait("Button \"Be tool wayland window — Test window\"");
    host.run(&["settle"]);
    host.snapshot("the_host_lists_a_programs_window_on_the_desktop");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.open_test_window("Test window");
    host.wait("Button \"Be tool wayland window — Test window\"");
    host.run(&["settle"]);
    host.snapshot("be_root_lists_a_programs_window_on_the_desktop");
    host.close();
}
