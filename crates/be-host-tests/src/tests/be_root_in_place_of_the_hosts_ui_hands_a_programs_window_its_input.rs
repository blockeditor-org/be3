use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_hands_a_programs_window_its_input() {
    // A press on a program window the desktop shows reaches its program, and gives it the
    // keyboard, whose keys reach it then, under be-host's own UI and, started again, under the
    // root alike: the root's router delivers what lands on the window's layer, which be-host
    // hands the compositor as the window's view would. The test program turns green once
    // pressed and red once a key reached it, and says so in its app id, which names its chip.
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    host.open_test_window("Test window");
    host.wait("Button \"Be tool wayland window — Test window\"");
    host.run(&["settle"]);
    host.click("#workspace.window");
    // be-host names a window by the last part of its app id, with its title.
    host.wait("Button \"Pressed — Test window\"");
    host.run(&["settle"]);
    host.key("a");
    host.wait("Button \"Typed — Test window\"");
    host.run(&["settle"]);
    host.snapshot("the_host_hands_a_programs_window_its_input");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.open_test_window("Test window");
    host.wait("Button \"be-tool-wayland-window\"");
    host.run(&["settle"]);
    host.click("#workspace.window");
    // The root's desktop names it by the last part of its app id alone (4g7e3d).
    host.wait("Button \"pressed\"");
    host.run(&["settle"]);
    host.key("a");
    host.wait("Button \"typed\"");
    host.run(&["settle"]);
    host.snapshot("be_root_hands_a_programs_window_its_input");
    host.close();
}
