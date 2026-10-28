use super::*;

const DELETED_BAR: &str = "GenericContainer #workspace.deleted at";

#[test]
fn a_deleted_joined_block_says_so_on_each_of_its_tabs() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    let more = row_menu(&mut app, "beui demo");
    app.click(&more);
    app.click("MenuItem \"Delete\"");
    app.wait("#workspace.deleted.restore");
    app.run(&["settle"]);
    assert_eq!(
        app.count(DELETED_BAR),
        2,
        "both of the block's tabs in the workspace dock show that it was deleted"
    );
    app.snapshot("a_deleted_joined_block_says_so_on_each_of_its_tabs");
    app.close();
}
