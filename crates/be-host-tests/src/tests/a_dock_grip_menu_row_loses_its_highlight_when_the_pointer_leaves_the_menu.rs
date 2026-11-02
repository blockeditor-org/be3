use super::*;

#[test]
fn a_dock_grip_menu_row_loses_its_highlight_when_the_pointer_leaves_the_menu() {
    let mut app = AppTest::start();
    let ids = app.run(&["ids"]);
    let grip = ids
        .split_whitespace()
        .map(|id| id.trim_start_matches('#'))
        .find(|id| id.starts_with("dock.grip."))
        .map(|id| format!("#{id}"))
        .expect("the files pane has a grip in the workspace dock");
    app.run(&["click", &grip, "right"]);
    let row = "MenuItemCheckBox \"Icons only\"";
    app.wait(row);
    app.run(&["move", row]);
    app.run(&["move", "Tab \"Settings\""]);
    app.wait(row);
    app.snapshot("a_dock_grip_menu_row_loses_its_highlight_when_the_pointer_leaves_the_menu");
    app.key("Escape");
    app.gone(row);
    app.close();
}
