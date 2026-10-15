use super::*;

#[test]
fn the_app_menu_opens_the_beui_demo() {
    let mut app = AppTest::start();
    app.click("#file-tree.desktop-app-menu");
    app.click("#app.menu.panel.BeuiDemo");
    app.wait("#workspace.host-panel");
    app.wait("Components");
    app.crop("#workspace.host-panel");
    app.snapshot("the_app_menu_opens_the_beui_demo");
    app.close();
}
