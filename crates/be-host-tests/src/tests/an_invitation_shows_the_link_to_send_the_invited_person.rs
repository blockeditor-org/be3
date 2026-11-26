use super::*;

#[test]
fn an_invitation_shows_the_link_to_send_the_invited_person() {
    let server = server();
    let dev = server.account("Dev");
    let mut host = HostTest::device(&dev);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.switch-organization");
    host.click("TextInput \"Organization name\"");
    host.type_text("Team");
    host.click("Button \"Create\"");
    host.wait("#file-tree.host-menu");
    host.click("#file-tree.host-menu");
    host.click("#host.menu.invite");
    // Nothing to fill in: the dialog makes the link at once and ends on it.
    host.wait("#host.invite.link");
    host.wait("#host.invite.qr");
    host.crop("#host.invite.help");
    host.snapshot("an_invitation_shows_the_link_to_send_the_invited_person");
    host.close();
}
