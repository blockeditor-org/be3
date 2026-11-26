use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// Opens the host menu from the files' button in a phone's window and paints it as
/// `painted`, then closes it with Escape, as
/// `on_a_phone_the_host_menu_rises_as_a_sheet_it_draws_itself` does.
fn rise_the_host_menu(host: &mut HostTest, painted: &str) {
    host.run(&["resize", "390x800"]);
    host.click("#file-tree.host-menu");
    host.wait("#host.menu.status");
    host.wait_still("#sheet.handle");
    host.synced();
    host.snapshot(painted);
    let (_, handle_y, _, _) = host.place("#sheet.handle");
    let (row_x, row_y, row_width, _) = host.place("#host.menu.status");
    assert!(
        handle_y > 40.0 && row_y > handle_y,
        "the menu rises from the bottom of the screen in a sheet, its grip at {handle_y}"
    );
    assert!(
        row_x < 20.0 && row_width > 350.0,
        "and its rows span the sheet, from {row_x} wide {row_width}"
    );
    host.key("Escape");
    host.run(&["pause", "600"]);
    host.gone("#host.menu.status");
}

#[test]
fn be_root_in_place_of_the_hosts_ui_rises_the_host_menu_as_a_sheet_on_a_phone() {
    // On a phone be-host's own UI draws its menu as a sheet rising from the bottom
    // (`on_a_phone_the_host_menu_rises_as_a_sheet_it_draws_itself`). Started again with the
    // root in place of its UI, the root draws the same sheet.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    rise_the_host_menu(&mut host, "the_host_rises_its_menu_as_a_sheet_on_a_phone");
    host.run(&["resize", "1100x720"]);
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    rise_the_host_menu(
        &mut host,
        "be_root_rises_the_host_menu_as_a_sheet_on_a_phone",
    );
    host.close();
}
