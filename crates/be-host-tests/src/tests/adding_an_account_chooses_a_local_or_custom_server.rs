use super::*;

#[test]
fn adding_an_account_chooses_a_local_or_custom_server() {
    let mut host = HostTest::start();
    host.click("#file-tree.host-menu");
    host.click("#host.menu.accounts");
    host.click("Button \"Add account\"");
    assert!(!host.tree().contains("#account.default"));
    host.crop("Dialog \"Add account\"");
    host.snapshot("adding_an_account_chooses_where_it_lives");
    host.click("#account.custom");
    host.wait("#account.server");
    assert!(!host.tree().contains("https://api.be3.pfg.pw"));
    host.crop("Dialog \"Connect to server\"");
    host.snapshot("adding_a_remote_account_asks_for_server_and_gateway");
    host.close();
}
