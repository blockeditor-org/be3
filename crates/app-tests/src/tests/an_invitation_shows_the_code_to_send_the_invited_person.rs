use super::*;

#[test]
fn an_invitation_shows_the_code_to_send_the_invited_person() {
    let mut app = AppTest::start();
    app.click("#file-tree.desktop-app-menu");
    app.click("#app.menu.invite");
    app.wait("Dialog \"Invite member\"");
    app.click("#app.invite.email");
    app.type_text("guest@example.com");
    app.click("#app.invite.send");
    app.wait("#app.invite.code");
    app.crop("#app.invite.help");
    app.snapshot("an_invitation_shows_the_code_to_send_the_invited_person");
    app.close();
}
