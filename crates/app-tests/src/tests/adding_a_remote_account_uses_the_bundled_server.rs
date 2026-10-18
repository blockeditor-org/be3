use super::*;

#[test]
fn adding_a_remote_account_uses_the_bundled_server() {
    let mut app = AppTest::start();
    app.click("#file-tree.desktop-app-menu");
    app.click("#app.menu.accounts");
    app.click("Button \"Add account\"");
    app.click("Tab \"Remote\"");
    app.wait("TextInput \"Server address\"");
    assert!(app.tree().contains("value=\"https://api.be3.pfg.pw\""));
    app.crop("Dialog \"Add account\"");
    app.snapshot("adding_a_remote_account_uses_the_bundled_server");
    app.close();
}
