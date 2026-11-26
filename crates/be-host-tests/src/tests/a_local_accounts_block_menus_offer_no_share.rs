use super::*;

/// A local account has no server to share through, so neither the files' section menu nor a
/// block's menu offers Share.
#[test]
fn a_local_accounts_block_menus_offer_no_share() {
    let mut host = HostTest::start(Options::default());
    host.click("#file-tree.host-menu");
    host.click("#host.menu.accounts");
    host.click("Button \"Add account\"");
    host.click("#account.local");
    host.click("#account.name");
    host.type_text("Ada");
    host.click("#account.confirm");
    host.click("Button \"Personal\"");
    host.wait("TreeItem \"My files\"");
    host.run(&["click", "TreeItem \"My files\"", "right"]);
    host.wait("#file-tree.section.rename");
    host.gone("#file-tree.section.share");
    host.key("Escape");
    host.gone("#file-tree.section.rename");

    add_to_my_files(&mut host, "Text");
    host.run(&["click", "TreeItem \"Text\"", "right"]);
    host.wait("#file-tree.appearance");
    let tree = host.tree();
    assert!(
        tree.contains("\"Rename\"") && !tree.contains("\"Share\""),
        "a local account's block menu offers Share:\n{tree}"
    );
    host.snapshot("a_local_accounts_block_menus_offer_no_share");
    host.close();
}
