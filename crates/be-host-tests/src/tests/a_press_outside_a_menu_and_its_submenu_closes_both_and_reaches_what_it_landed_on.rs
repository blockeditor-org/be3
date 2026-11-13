use super::*;

#[test]
fn a_press_outside_a_menu_and_its_submenu_closes_both_and_reaches_what_it_landed_on() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.wait("Tab \"Text\"");
    host.run(&["click", "TreeItem \"Text\"", "right"]);
    host.wait("#file-tree.export");
    let menu = host.count("MenuItem");
    host.click("#file-tree.export");
    host.run(&["settle"]);
    assert!(
        host.count("MenuItem") > menu,
        "the export submenu opens beside the menu"
    );
    host.snapshot("a_files_menu_and_its_export_submenu_reach_over_the_editor");

    host.click("Tab \"Settings\"");
    host.gone("#file-tree.export");
    host.wait("Tab \"Settings\" selected");
    host.close();
}
