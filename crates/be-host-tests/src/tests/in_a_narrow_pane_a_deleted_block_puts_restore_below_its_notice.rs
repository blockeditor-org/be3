use super::*;

#[test]
fn in_a_narrow_pane_a_deleted_block_puts_restore_below_its_notice() {
    let mut host = HostTest::start();
    open_beui_demo(&mut host);
    let more = row_menu(&mut host, "beui demo");
    host.click(&more);
    host.click("MenuItem \"Delete\"");
    host.wait("#workspace.deleted.restore");
    host.run(&["resize", "360x800"]);
    host.run(&["settle"]);
    let bar = host.place("#workspace.deleted");
    let restore = host.place("#workspace.deleted.restore");
    assert!(
        restore.0 + restore.2 <= bar.0 + bar.2 && restore.1 > bar.1 + restore.3 / 2.0,
        "a notice too narrow for its caption and its action puts the action below the caption, \
         inside the bar: Restore at {restore:?} in {bar:?}"
    );
    host.snapshot("in_a_narrow_pane_a_deleted_block_puts_restore_below_its_notice");
    host.close();
}
