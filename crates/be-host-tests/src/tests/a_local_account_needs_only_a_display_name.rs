use super::*;

#[test]
fn a_local_account_needs_only_a_display_name() {
    let mut host = HostTest::start();
    host.click("#file-tree.host-menu");
    host.click("#host.menu.accounts");
    host.click("Button \"Add account\"");
    host.click("#account.local");
    host.click("#account.name");
    host.type_text("Ada");
    let tree = host.tree();
    assert!(!tree.contains("Email address"));
    assert!(!tree.contains("Password"));
    host.crop("Dialog \"Create local account\"");
    host.snapshot("a_local_account_needs_only_a_display_name");
    host.click("#account.confirm");
    host.wait("Button \"Personal\"");
    assert!(!host.tree().contains("Check that you saved them"));
    host.close();
}
