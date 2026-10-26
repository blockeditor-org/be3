use super::*;

#[test]
fn a_tab_popped_out_in_space_is_a_panel_of_its_own() {
    let mut app = AppTest::start_in_space();
    app.run(&["app", "click", "Tab \"Settings\"", "right"]);
    app.run(&["app", "click", "MenuItem \"Pop out into a window\""]);
    app.wait("Pane \"Settings\"");
    app.snapshot("a_tab_popped_out_in_space_is_a_panel_of_its_own");
    app.close();
}
