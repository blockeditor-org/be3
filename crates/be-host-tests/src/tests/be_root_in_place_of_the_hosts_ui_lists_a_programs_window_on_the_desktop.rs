use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_lists_a_programs_window_on_the_desktop() {
    // A program run in the desktop's compositor opens a window, which the desktop's windows bar
    // lists, under be-host's own UI and, started again, under the root alike: the root drives
    // the desktop host as be-host's UI does, and the window reaches the desktop app, the shell,
    // which opens it in a tab. Under the root the tab shows nothing of the window yet, and both
    // are named by the program rather than its window's title, which reaches the desktop only
    // as its child's status (plans/host_as_platform_adapter.md, 4g7e3b and 4g7e3d).
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    host.open_test_window("Test window");
    // Titled once the program says its title.
    host.wait("Button \"Be tool wayland window — Test window\"");
    host.run(&["settle"]);
    // The windows bar only: drive's settle does not wait for a program's first frame, so the
    // window's own content may still be arriving.
    host.crop("0,676,1100,44");
    host.snapshot("the_host_lists_a_programs_window_on_the_desktop");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.open_test_window("Test window");
    // The window's own chip, named by its program: the bar's "No windows" placeholder is a
    // `#desktop.windows.` target too.
    host.wait("Button \"be-tool-wayland-window\"");
    host.run(&["settle"]);
    host.crop("0,676,1100,44");
    host.snapshot("be_root_lists_a_programs_window_on_the_desktop");
    host.close();
}
