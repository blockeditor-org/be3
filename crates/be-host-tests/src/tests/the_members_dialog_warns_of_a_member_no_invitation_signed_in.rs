use super::*;

/// A member the server lists that no invitation signed into the organization's log (one the
/// server added on its own, since accepting an invitation signs a person in) shows with a
/// warning to administrators.
#[test]
fn the_members_dialog_warns_of_a_member_no_invitation_signed_in() {
    let server = Server::start();
    let dev = server.account("Dev");
    let joiner = server.account("Joiner");
    let mut host = HostTest::device(&dev);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.switch-organization");
    host.click("TextInput \"Organization name\"");
    host.type_text("Team");
    host.click("Button \"Create\"");
    host.wait("#file-tree.host-menu");
    server.add_member(&dev, "Team", &joiner);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.members");
    host.wait("Dialog \"Members\"");
    host.wait("Text \"No invitation signed them in: the server added them. Remove them unless you know why they are here.\"");
    host.crop("Dialog \"Members\"");
    host.snapshot("the_members_dialog_warns_of_a_member_no_invitation_signed_in");
    host.close();
}
