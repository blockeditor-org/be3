use super::*;

const DELETED_BAR: &str = "GenericContainer #workspace.deleted at";

#[test]
fn a_deleted_joined_block_says_so_on_each_of_its_tabs() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    let row = app.place("TreeItem \"beui demo\"");
    let more = format!("{:.0},{:.0}", row.0 + row.2 - 14.0, row.1 + row.3 * 0.5);
    app.run(&["click", &more]);
    app.click("MenuItem \"Delete\"");
    app.wait("Button \"Restore\" #workspace.deleted.restore at 771");
    app.run(&["settle"]);
    assert_eq!(
        app.count(DELETED_BAR),
        2,
        "both of the block's tabs in the workspace dock show that it was deleted"
    );
    app.snapshot("a_deleted_joined_block_says_so_on_each_of_its_tabs");
    app.close();
}
