use super::*;

#[test]
fn the_organizations_screen_takes_an_invite_link() {
    let mut app = AppTest::start();
    app.click("#file-tree.app-menu");
    app.click("#app.menu.switch-organization");
    app.wait("#app.join.link");
    app.crop("#app.join");
    app.snapshot("the_organizations_screen_takes_an_invite_link");
    app.close();
}
