use super::*;

#[test]
fn a_tab_popped_out_in_space_is_a_panel_of_its_own() {
    let mut host = HostTest::start(Options {
        space: true,
        ..Options::default()
    });
    host.run(&["click", "#dock.grip.files", "right"]);
    host.run(&["click", "MenuItemCheckBox \"Locked\""]);
    host.run(&["click", "Tab \"Settings\"", "right"]);
    host.run(&["click", "MenuItem \"Pop out into a window\""]);
    host.wait("Pane \"Settings\"");
    host.snapshot("a_tab_popped_out_in_space_is_a_panel_of_its_own");
    host.close();
}
