use super::*;

#[test]
fn on_a_phone_the_app_menu_rises_as_a_sheet_it_draws_itself() {
    let mut app = AppTest::start();
    app.run(&["resize", "390x800"]);
    app.run(&["clock", "run"]);
    app.click("#file-tree.app-menu");
    app.wait("#app.menu.status");
    app.run(&["pause", "600"]);
    app.run(&["clock", "stop"]);
    app.snapshot("on_a_phone_the_app_menu_rises_as_a_sheet_it_draws_itself");
    let (_, handle_y, _, _) = app.place("#sheet.handle");
    let (row_x, row_y, row_width, _) = app.place("#app.menu.status");
    assert!(
        handle_y > 40.0 && row_y > handle_y,
        "the menu rises from the bottom of the screen in a sheet, its grip at {handle_y}"
    );
    assert!(
        row_x < 20.0 && row_width > 350.0,
        "and its rows span the sheet, from {row_x} wide {row_width}"
    );

    app.run(&["clock", "run"]);
    app.key("Escape");
    app.gone("#app.menu.status");
    app.close();
}
