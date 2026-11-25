use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_draws_a_host_panel_the_workspace_places() {
    // With the root in place of be-host's UI, the files' Account and settings button opens the
    // host's menu panel in a popover, which the root draws itself in the place the files give
    // it. Its Performance row asks the workspace to show that panel, as be-host's does: the
    // workspace places it as a tab, and the root draws the host's statistics there.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    let button = host.place("#file-tree.host-menu");
    let button = format!(
        "{:.0},{:.0}",
        button.0 + button.2 / 2.0,
        button.1 + button.3 / 2.0
    );
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.run(&["click", &button]);
    host.run(&["settle"]);
    host.snapshot("be_root_draws_the_hosts_menu_panel");
    // Its rows scroll in the popover's room: Performance, near its end, scrolled into view and
    // clicked where it then shows.
    host.run(&["scroll", "315,250", "300"]);
    host.run(&["settle"]);
    host.run(&["click", "298,352"]);
    host.run(&["settle"]);
    // The menu's popover closed by a press outside it, the panel's tab and title: what it
    // shows of the host's statistics changes from run to run, so only its top is painted.
    host.run(&["click", "900,650"]);
    host.run(&["settle"]);
    host.crop("120,100,520,72");
    host.snapshot("be_root_draws_the_hosts_performance_panel");
    host.close();
}
