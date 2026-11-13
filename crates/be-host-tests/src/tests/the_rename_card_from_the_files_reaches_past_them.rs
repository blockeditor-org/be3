use super::*;

#[test]
fn the_rename_card_from_the_files_reaches_past_them() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.wait("Tab \"Text\"");
    host.run(&["click", "TreeItem \"Text\"", "right"]);
    host.click("#file-tree.appearance");
    host.wait("#rename.search");
    let (_, row_y, _, row_height) = host.place("TreeItem \"Text\"");
    let (files_x, _, files_width, _) = host.place("#workspace.files");
    let (search_x, search_y, search_width, _) = host.place("#rename.search");
    assert!(
        search_y > row_y + row_height,
        "the card opens under the row that asked: {search_y} under {row_y} + {row_height}"
    );
    assert!(
        search_x + search_width > files_x + files_width,
        "the card keeps its size and reaches past the files: \
         {search_x} + {search_width} past {files_x} + {files_width}"
    );
    host.snapshot("the_rename_card_from_the_files_reaches_past_them");

    host.click("#rename.color.pink");
    assert!(
        host.shown("#rename.search"),
        "picking a color leaves the card open"
    );
    host.key("Escape");
    host.gone("#rename.search");

    host.run(&["click", "TreeItem \"Text\"", "right"]);
    host.click("#file-tree.appearance");
    host.wait("#rename.search");
    host.click("Tab \"Settings\"");
    host.gone("#rename.search");
    host.close();
}
