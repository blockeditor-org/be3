use super::*;

#[test]
fn adding_an_account_chooses_a_local_or_custom_server() {
    let mut app = AppTest::start();
    app.click("#file-tree.app-menu");
    app.click("#app.menu.accounts");
    app.click("Button \"Add account\"");
    assert!(!app.tree().contains("#account.default"));
    app.crop("Dialog \"Add account\"");
    app.snapshot("adding_an_account_chooses_where_it_lives");
    app.click("#account.custom");
    app.wait("#account.server");
    assert!(!app.tree().contains("https://api.be3.pfg.pw"));
    app.crop("Dialog \"Connect to server\"");
    app.snapshot("adding_a_remote_account_asks_for_server_and_gateway");
    app.close();
}
