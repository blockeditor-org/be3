use super::*;

#[test]
fn a_press_outside_a_menu_and_its_submenu_closes_both_and_reaches_what_it_landed_on() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.wait("Tab \"Text\"");
    app.run(&["click", "TreeItem \"Text\"", "right"]);
    app.wait("#file-tree.export");
    let menu = app.count("MenuItem");
    app.click("#file-tree.export");
    app.run(&["settle"]);
    assert!(
        app.count("MenuItem") > menu,
        "the export submenu opens beside the menu"
    );
    app.snapshot("a_files_menu_and_its_export_submenu_reach_over_the_editor");

    app.click("Tab \"Settings\"");
    app.gone("#file-tree.export");
    app.wait("Tab \"Settings\" selected");
    app.close();
}
