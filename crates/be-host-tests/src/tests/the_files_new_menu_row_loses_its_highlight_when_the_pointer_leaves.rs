use super::*;

#[test]
fn the_files_new_menu_row_loses_its_highlight_when_the_pointer_leaves() {
    let mut host = HostTest::start(Options::default());
    host.click("#file-tree.new");
    let row = "#file-tree.new-workspace";
    host.wait(row);
    host.run(&["move", row]);
    host.run(&["move", "\"No file open\""]);
    host.wait(row);
    host.snapshot("the_files_new_menu_row_loses_its_highlight_when_the_pointer_leaves_the_menu");

    host.run(&["move", row]);
    host.run(&["leave"]);
    host.wait(row);
    host.snapshot("the_files_new_menu_row_loses_its_highlight_when_the_pointer_leaves_the_window");
    host.key("Escape");
    host.gone(row);
    host.close();
}
