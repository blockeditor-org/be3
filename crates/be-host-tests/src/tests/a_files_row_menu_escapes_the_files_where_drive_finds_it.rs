use super::*;

#[test]
fn a_files_row_menu_escapes_the_files_where_drive_finds_it() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.wait("Tab \"Text\"");
    let (files_x, _, files_width, _) = host.place("#workspace.files");
    let (row_x, row_y, row_width, row_height) = host.place("TreeItem \"Text\"");
    let press = (row_x + row_width - 12.0, row_y + row_height / 2.0);
    host.run(&["click", &format!("{:.0},{:.0}", press.0, press.1), "right"]);
    host.wait("#file-tree.appearance");
    let (item_x, item_y, item_width, _) = host.place("#file-tree.zoom-in");
    assert!(
        (item_x - press.0).abs() < 12.0 && (item_y - press.1).abs() < 12.0,
        "the menu is described where it opened, at the press: {item_x},{item_y} from {press:?}"
    );
    assert!(
        item_x + item_width > files_x + files_width,
        "the menu reaches past the files: {item_x} + {item_width} past {files_x} + {files_width}"
    );
    host.snapshot("a_files_row_menu_escapes_the_files_where_drive_finds_it");
    host.click("#file-tree.appearance");
    host.wait("#rename.search");
    host.key("Escape");
    host.gone("#rename.search");
    host.close();
}
