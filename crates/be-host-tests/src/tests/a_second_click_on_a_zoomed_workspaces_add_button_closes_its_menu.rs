use super::*;

#[test]
fn a_second_click_on_a_zoomed_workspaces_add_button_closes_its_menu() {
    let mut app = AppTest::start();
    app.click("TreeItem \"My files\"");
    let add = "#file-tree.new";
    app.click(add);
    app.wait("#picker.more");
    app.click(add);
    assert!(
        !app.shown("#picker.more"),
        "a second click on the add button closes the menu it opened rather than closing it \
         and loading it again"
    );
    app.click(add);
    app.wait("#picker.more");
    app.close();
}
