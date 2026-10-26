use super::*;

#[test]
fn a_tab_popped_out_in_space_is_a_panel_of_its_own() {
    let mut app = AppTest::start_in_space();
    app.run(&["click", "Tab \"Settings\"", "right"]);
    app.run(&["click", "MenuItem \"Pop out into a window\""]);
    app.gone("MenuItem \"Pop out into a window\"");
    app.run(&["drag", "300,57", "600,300", "4"]);
    app.snapshot("a_tab_popped_out_in_space_is_a_panel_of_its_own");
    app.close();
}
