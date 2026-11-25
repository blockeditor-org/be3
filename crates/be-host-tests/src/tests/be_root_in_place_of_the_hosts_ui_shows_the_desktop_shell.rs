use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_the_desktop_shell() {
    // be-host's own UI runs the desktop; started again with the root in place of its UI and
    // `--desktop` still given, the root shows the desktop shell as its session's editor, its
    // bars along the display's edges, as be-host's own UI shows it.
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    host.snapshot("the_host_shows_the_desktop_shell");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_the_desktop_shell");
    host.close();
}
