use super::*;

#[test]
fn the_appearance_card_from_the_files_reaches_past_them() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.wait("Tab \"Text\"");
    app.run(&["click", "TreeItem \"Text\"", "right"]);
    app.click("#file-tree.appearance");
    app.wait("#appearance.search");
    let (_, row_y, _, row_height) = app.place("TreeItem \"Text\"");
    let (files_x, _, files_width, _) = app.place("#workspace.files");
    let (search_x, search_y, search_width, _) = app.place("#appearance.search");
    assert!(
        search_y > row_y + row_height,
        "the card opens under the row that asked: {search_y} under {row_y} + {row_height}"
    );
    assert!(
        search_x + search_width > files_x + files_width,
        "the card keeps its size and reaches past the files: \
         {search_x} + {search_width} past {files_x} + {files_width}"
    );
    app.snapshot("the_appearance_card_from_the_files_reaches_past_them");

    app.click("#appearance.color.pink");
    assert!(
        app.shown("#appearance.search"),
        "picking a color leaves the card open"
    );
    app.key("Escape");
    app.gone("#appearance.search");
    app.close();
}
