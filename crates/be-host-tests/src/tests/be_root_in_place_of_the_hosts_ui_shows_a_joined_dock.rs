use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_a_joined_dock() {
    // be-host's own UI makes a beui demo block and opens it in a workspace tab; started again
    // with the root in place of its UI, the workspace the root shows places the demo in that
    // tab's body. The root asks the demo's editor for its frame docked, the demo's dock joins
    // the workspace's in place of the tab (its Components and Docking tabs in the workspace's
    // tab bar), and the root shows the bodies the workspace places for them as the demo's own
    // views.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    open_beui_demo(&mut host);
    // Time for its name to be kept, which the root's Files and tab show.
    host.pause(1000);
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_a_joined_dock");
    host.close();
}
