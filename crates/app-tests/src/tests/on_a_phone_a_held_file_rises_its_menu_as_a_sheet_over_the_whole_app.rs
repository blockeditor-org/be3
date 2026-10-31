use super::*;

#[test]
fn on_a_phone_a_held_file_rises_its_menu_as_a_sheet_over_the_whole_app() {
    let mut app = AppTest::start();
    app.run(&["resize", "390x800"]);
    app.wait("TreeItem \"My files\"");
    let (x, y, width, height) = app.place("TreeItem \"My files\"");
    let held = format!("{:.0},{:.0}", x + width / 2.0, y + height / 2.0);
    app.run(&["touch", "down", &held]);
    app.run(&["pause", "900"]);
    app.run(&["touch", "up"]);
    app.run(&["pause", "900"]);
    let (_, files_y, _, _) = app.place("Pane \"Files\"");
    let (outside_x, outside_y, outside_width, _) = app.place("#sheet.outside");
    let (row_x, row_y, row_width, row_height) = app.place("#file-tree.section.settings");
    assert!(
        outside_y < files_y && outside_x <= 0.0 && outside_width >= 390.0,
        "the sheet's layer spans the whole app, its top bar too, not just the files: from {outside_y}, the files from {files_y}"
    );
    assert!(
        row_y + row_height > 760.0 && row_x < 20.0 && row_width > 350.0,
        "and its rows rise from the bottom of the window across its width"
    );
    app.snapshot("on_a_phone_a_held_file_rises_its_menu_as_a_sheet_over_the_whole_app");
    app.run(&["tap", "200,200"]);
    app.run(&["pause", "600"]);
    app.gone("#file-tree.section.settings");
    app.close();
}
