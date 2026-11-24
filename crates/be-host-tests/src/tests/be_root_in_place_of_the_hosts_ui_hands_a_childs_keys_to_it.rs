use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// The Files' filter field, in the side panel the workspace places them in, in the window's
/// points.
const FILTER: &str = "85,64";

#[test]
fn be_root_in_place_of_the_hosts_ui_hands_a_childs_keys_to_it() {
    // A press in the Files, a child the workspace places live, gives the Files' app the
    // keyboard: the keys typed then reach its filter field, not the workspace around it. What
    // the workspace hears of it (`child-focused`) and an editor's own ask (`keyboard-ask`) are
    // covered by be-sdk-root's `children::tests`: under the root, the workspace asks only for
    // the children of its dock's tabs, whose bodies the root does not show yet (4g7b3c).
    let mut host = HostTest::root_with_dev_workspace(ROOT);
    host.run(&["settle"]);
    host.click(FILTER);
    host.type_text("Shared");
    host.run(&["settle"]);
    host.snapshot("be_root_hands_a_childs_keys_to_it");
    host.close();
}
