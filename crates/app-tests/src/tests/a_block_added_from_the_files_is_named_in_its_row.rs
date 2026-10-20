use super::*;

#[test]
fn a_block_added_from_the_files_is_named_in_its_row() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.wait("TextInput \"Name\" value=\"Text\" focused");
    app.crop("#workspace.files");
    app.snapshot("a_block_added_from_the_files_is_named_in_its_row");
    app.type_text("Plans");
    app.key("Enter");
    app.wait("TreeItem \"Plans\"");
    app.wait("Tab \"Plans\"");
    app.close();
}
