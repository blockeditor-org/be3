use super::*;

#[test]
fn a_dock_grip_menu_row_loses_its_highlight_when_the_pointer_leaves_the_menu() {
    let mut host = HostTest::start(Options::default());
    let ids = host.run(&["ids"]);
    let grip = ids
        .split_whitespace()
        .map(|id| id.trim_start_matches('#'))
        .find(|id| id.starts_with("dock.grip."))
        .map(|id| format!("#{id}"))
        .expect("the files pane has a grip in the workspace dock");
    host.run(&["click", &grip, "right"]);
    let row = "MenuItemCheckBox \"Icons only\"";
    host.wait(row);
    host.run(&["move", row]);
    host.run(&["move", "Tab \"Settings\""]);
    host.wait(row);
    host.snapshot("a_dock_grip_menu_row_loses_its_highlight_when_the_pointer_leaves_the_menu");
    host.key("Escape");
    host.gone(row);
    host.close();
}
