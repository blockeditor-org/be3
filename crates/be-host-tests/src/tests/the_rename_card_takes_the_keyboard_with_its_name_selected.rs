use super::*;

#[test]
fn the_rename_card_takes_the_keyboard_with_its_name_selected() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.wait("Tab \"Text\"");
    host.run(&["click", "TreeItem \"Text\"", "right"]);
    host.click("#file-tree.appearance");
    host.wait("#rename.name");
    host.type_text("Notes");
    host.wait("TreeItem \"Notes\"");
    host.key("Enter");
    host.gone("#rename.name");
    host.close();
}
