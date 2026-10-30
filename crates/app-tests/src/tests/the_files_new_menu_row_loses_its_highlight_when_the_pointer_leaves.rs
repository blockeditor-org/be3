use super::*;

#[test]
fn the_files_new_menu_row_loses_its_highlight_when_the_pointer_leaves() {
    let mut app = AppTest::start();
    app.click("#file-tree.new");
    let row = "#file-tree.new-workspace";
    app.wait(row);
    app.run(&["move", row]);
    app.run(&["move", "\"No file open\""]);
    app.wait(row);
    app.snapshot("the_files_new_menu_row_loses_its_highlight_when_the_pointer_leaves_the_menu");

    app.run(&["move", row]);
    app.run(&["leave"]);
    app.wait(row);
    app.snapshot("the_files_new_menu_row_loses_its_highlight_when_the_pointer_leaves_the_window");
    app.key("Escape");
    app.gone(row);
    app.close();
}
