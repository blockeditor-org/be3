use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_warns_of_a_member_no_invitation_signed_in() {
    // A member the server added on its own shows in the root's Members with be-host's warning
    // (the_members_dialog_warns_of_a_member_no_invitation_signed_in).
    let server = server();
    let dev = server.account("Dev");
    let joiner = server.account("Joiner");
    let mut host = HostTest::drawn_device(&dev);
    create_organization(&mut host, "Team");
    server.add_member(&dev, "Team", &joiner);
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.members");
    host.wait("Dialog \"Members\"");
    host.wait("Label value=\"No invitation signed them in: the server added them. Remove them unless you know why they are here.\"");
    host.crop("Dialog \"Members\"");
    host.snapshot("be_root_warns_of_a_member_no_invitation_signed_in");
    host.close();
}
