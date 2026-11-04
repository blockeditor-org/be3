use super::*;

#[test]
fn a_second_click_on_a_zoomed_workspaces_add_button_closes_its_menu() {
    let mut host = HostTest::start();
    host.click("TreeItem \"My files\"");
    let add = "#file-tree.new";
    host.click(add);
    host.wait("#picker.more");
    host.click(add);
    assert!(
        !host.shown("#picker.more"),
        "a second click on the add button closes the menu it opened rather than closing it \
         and loading it again"
    );
    host.click(add);
    host.wait("#picker.more");
    host.close();
}
