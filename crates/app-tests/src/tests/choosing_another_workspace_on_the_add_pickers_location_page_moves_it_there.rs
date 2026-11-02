use super::*;

#[test]
fn choosing_another_workspace_on_the_add_pickers_location_page_moves_it_there() {
    let mut host = HostTest::start();
    host.click(NEW);
    host.click("#file-tree.new-workspace");
    host.wait("TextInput \"Name\" value=\"New workspace\" focused");
    host.type_text("Team");
    host.key("Enter");
    host.wait("TreeItem \"Team\"");

    open_add_to_my_files(&mut host);
    host.wait("MenuItem \"New in My files\"");
    host.click("#picker.location");
    host.click("MenuItemCheckBox \"Team\"");
    host.wait("MenuItem \"New in Team\"");
    host.key("Escape");
    host.gone("#picker.more");
    host.close();
}
