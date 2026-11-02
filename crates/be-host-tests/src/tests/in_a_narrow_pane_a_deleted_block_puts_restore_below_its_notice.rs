use super::*;

#[test]
fn in_a_narrow_pane_a_deleted_block_puts_restore_below_its_notice() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    let more = row_menu(&mut app, "beui demo");
    app.click(&more);
    app.click("MenuItem \"Delete\"");
    app.wait("#workspace.deleted.restore");
    app.run(&["resize", "360x800"]);
    app.run(&["settle"]);
    let bar = app.place("#workspace.deleted");
    let restore = app.place("#workspace.deleted.restore");
    assert!(
        restore.0 + restore.2 <= bar.0 + bar.2 && restore.1 > bar.1 + restore.3 / 2.0,
        "a notice too narrow for its caption and its action puts the action below the caption, \
         inside the bar: Restore at {restore:?} in {bar:?}"
    );
    app.snapshot("in_a_narrow_pane_a_deleted_block_puts_restore_below_its_notice");
    app.close();
}
