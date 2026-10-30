use super::*;

#[test]
fn an_invitation_shows_the_link_to_send_the_invited_person() {
    let mut app = AppTest::start();
    app.click("#file-tree.app-menu");
    app.click("#app.menu.switch-organization");
    app.click("TextInput \"Organization name\"");
    app.type_text("Team");
    app.click("Button \"Create\"");
    app.wait("#file-tree.app-menu");
    app.click("#file-tree.app-menu");
    app.click("#app.menu.invite");
    app.wait("Dialog \"Invite member\"");
    app.click("#app.invite.email");
    app.type_text("guest@example.com");
    app.click("#app.invite.send");
    app.wait("#app.invite.link");
    app.crop("#app.invite.help");
    app.snapshot("an_invitation_shows_the_link_to_send_the_invited_person");
    app.close();
}
