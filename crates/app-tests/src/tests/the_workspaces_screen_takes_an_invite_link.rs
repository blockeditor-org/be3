use super::*;

#[test]
fn the_workspaces_screen_takes_an_invite_link() {
    let mut app = AppTest::start();
    app.click("#file-tree.desktop-app-menu");
    app.click("#app.menu.switch-workspace");
    app.wait("#app.join.link");
    app.crop("#app.join");
    app.snapshot("the_workspaces_screen_takes_an_invite_link");
    app.close();
}
