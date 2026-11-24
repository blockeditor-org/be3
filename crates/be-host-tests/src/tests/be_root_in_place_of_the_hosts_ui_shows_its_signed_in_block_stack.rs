use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// Where the root's menu button is, in the window's points: a square of `be_root::MENU_BUTTON`
/// at the window's top right (1100 points wide).
const MENU_BUTTON: &str = "1076,24";

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_its_signed_in_block_stack() {
    // be-host runs its block stack under the root, signed in to the dev workspace: the root
    // waits for nothing (no onboarding over the window) and its menu names the organization
    // open and who is signed in, from be-host's block stack through be3:root.
    let mut host = HostTest::root_with_dev_workspace(ROOT);
    host.run(&["settle"]);
    host.snapshot("be_root_signed_in");
    host.click(MENU_BUTTON);
    host.run(&["settle"]);
    host.snapshot("be_root_signed_in_menu");
    host.close();
}
