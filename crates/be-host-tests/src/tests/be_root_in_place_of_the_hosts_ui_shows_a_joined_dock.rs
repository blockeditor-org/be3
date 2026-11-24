use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

// A child taking its placer's frame over has no painting under the root yet: be-host's UI has
// no child that takes a frame and shows under the root. A canvas places a card's editor as a
// preview until focused, and focuses it only once it is available, but the root shows no
// previews (plans/host_as_platform_adapter.md, 4g7b3d), so a double-click in the card enters
// nothing. be-sdk-root's `children::tests` cover it until then.
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
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_a_joined_dock");
    host.close();
}
