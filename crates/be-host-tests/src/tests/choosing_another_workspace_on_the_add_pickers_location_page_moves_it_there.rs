use super::*;

#[test]
fn choosing_another_workspace_on_the_add_pickers_location_page_moves_it_there() {
    let mut app = AppTest::start();
    app.click(NEW);
    app.click("#file-tree.new-workspace");
    app.wait("TextInput \"Name\" value=\"New workspace\" focused");
    app.type_text("Team");
    app.key("Enter");
    app.wait("TreeItem \"Team\"");

    open_add_to_my_files(&mut app);
    app.wait("MenuItem \"New in My files\"");
    app.click("#picker.location");
    app.click("MenuItemCheckBox \"Team\"");
    app.wait("MenuItem \"New in Team\"");
    app.key("Escape");
    app.gone("#picker.more");
    app.close();
}
