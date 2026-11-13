use super::*;

const DELETED_BAR: &str = "GenericContainer #workspace.deleted at";

#[test]
fn a_deleted_joined_block_says_so_on_each_of_its_tabs() {
    let mut host = HostTest::start(Options::default());
    open_beui_demo(&mut host);
    let more = row_menu(&mut host, "beui demo");
    host.click(&more);
    host.click("MenuItem \"Delete\"");
    host.wait("#workspace.deleted.restore");
    host.run(&["settle"]);
    assert_eq!(
        host.count(DELETED_BAR),
        2,
        "both of the block's tabs in the workspace dock show that it was deleted"
    );
    host.snapshot("a_deleted_joined_block_says_so_on_each_of_its_tabs");
    host.close();
}
