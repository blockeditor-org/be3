use super::*;

#[test]
fn on_a_phone_the_host_menu_rises_as_a_sheet_it_draws_itself() {
    let mut host = HostTest::start();
    host.run(&["resize", "390x800"]);
    host.click("#file-tree.host-menu");
    host.wait("#host.menu.status");
    host.run(&["pause", "600"]);
    host.snapshot("on_a_phone_the_host_menu_rises_as_a_sheet_it_draws_itself");
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
    host.close();
}
