use super::*;

/// A member the server lists that no administrator's device signed into the organization's log
/// shows as not signed in, with a way to sign them in (signed access 3a-members): until one
/// does, administrators open none of their escrow wraps.
#[test]
fn the_members_dialog_offers_to_sign_in_a_member_no_one_signed_in() {
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
    host.wait("Button \"Sign in\"");
    host.crop("Dialog \"Members\"");
    host.snapshot("the_members_dialog_offers_to_sign_in_a_member_no_one_signed_in");
    host.close();
}
