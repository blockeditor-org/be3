use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_lists_an_organizations_members_and_their_roles() {
    // be-host's own UI makes an organization; started again with the root in place of its UI,
    // the root's Members lists its members and their roles as be-host's does
    // (the_members_dialog_lists_an_organizations_members_and_their_roles).
    let server = server();
    let dev = server.account("Dev");
    let mut host = HostTest::drawn_device(&dev);
    create_organization(&mut host, "Team");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.members");
    host.wait("Dialog \"Members\"");
    host.wait("Label value=\"dev@example.com, Administrator\"");
    host.crop("Dialog \"Members\"");
    host.snapshot("be_root_lists_an_organizations_members_and_their_roles");
    host.close();
}
