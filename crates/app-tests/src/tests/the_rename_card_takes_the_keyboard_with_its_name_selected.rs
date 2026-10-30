use super::*;

#[test]
fn the_rename_card_takes_the_keyboard_with_its_name_selected() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.wait("Tab \"Text\"");
    app.run(&["click", "TreeItem \"Text\"", "right"]);
    app.click("#file-tree.appearance");
    app.wait("#rename.name");
    app.type_text("Notes");
    app.wait("TreeItem \"Notes\"");
    app.key("Enter");
    app.gone("#rename.name");
    app.close();
}
