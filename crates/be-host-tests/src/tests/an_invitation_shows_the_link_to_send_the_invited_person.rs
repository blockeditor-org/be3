use super::*;

#[test]
fn an_invitation_shows_the_link_to_send_the_invited_person() {
    let mut host = HostTest::start(Options::default());
    host.click("#file-tree.host-menu");
    host.click("#host.menu.switch-organization");
    host.click("TextInput \"Organization name\"");
    host.type_text("Team");
    host.click("Button \"Create\"");
    host.wait("#file-tree.host-menu");
    host.click("#file-tree.host-menu");
    host.click("#host.menu.invite");
    host.wait("Dialog \"Invite member\"");
    host.click("#host.invite.email");
    host.type_text("guest@example.com");
    host.click("#host.invite.send");
    host.wait("#host.invite.link");
    host.crop("#host.invite.help");
    host.snapshot("an_invitation_shows_the_link_to_send_the_invited_person");
    host.close();
}
