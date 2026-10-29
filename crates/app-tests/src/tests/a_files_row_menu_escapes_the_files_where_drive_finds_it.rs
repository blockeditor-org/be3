use super::*;

#[test]
fn a_files_row_menu_escapes_the_files_where_drive_finds_it() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.wait("Tab \"Text\"");
    let (files_x, _, files_width, _) = app.place("#workspace.files");
    let (row_x, row_y, row_width, row_height) = app.place("TreeItem \"Text\"");
    let press = (row_x + row_width - 12.0, row_y + row_height / 2.0);
    app.run(&["click", &format!("{:.0},{:.0}", press.0, press.1), "right"]);
    app.wait("#file-tree.appearance");
    let (item_x, item_y, item_width, _) = app.place("#file-tree.zoom-in");
    assert!(
        (item_x - press.0).abs() < 12.0 && (item_y - press.1).abs() < 12.0,
        "the menu is described where it opened, at the press: {item_x},{item_y} from {press:?}"
    );
    assert!(
        item_x + item_width > files_x + files_width,
        "the menu reaches past the files: {item_x} + {item_width} past {files_x} + {files_width}"
    );
    app.snapshot("a_files_row_menu_escapes_the_files_where_drive_finds_it");
    app.click("#file-tree.appearance");
    app.wait("#rename.search");
    app.key("Escape");
    app.gone("#rename.search");
    app.close();
}
