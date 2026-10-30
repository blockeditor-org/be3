use super::*;

#[test]
fn a_local_account_needs_only_a_display_name() {
    let mut app = AppTest::start();
    app.click("#file-tree.app-menu");
    app.click("#app.menu.accounts");
    app.click("Button \"Add account\"");
    app.click("#account.local");
    app.click("#account.name");
    app.type_text("Ada");
    let tree = app.tree();
    assert!(!tree.contains("Email address"));
    assert!(!tree.contains("Password"));
    app.crop("Dialog \"Create local account\"");
    app.snapshot("a_local_account_needs_only_a_display_name");
    app.click("#account.confirm");
    app.wait("Label value=\"Check that you saved them\"");
    app.close();
}
