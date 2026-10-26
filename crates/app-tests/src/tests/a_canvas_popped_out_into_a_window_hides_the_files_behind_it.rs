use super::*;

#[test]
fn a_canvas_popped_out_into_a_window_hides_the_files_behind_it() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.wait("Pane \"Canvas\"");
    app.run(&["click", "Tab \"Canvas\"", "right"]);
    app.run(&["click", "MenuItem \"Pop out into a window\""]);
    app.wait("Tab \"Canvas\"");
    let window = app.place("Pane \"Canvas\"");
    let edge = format!(
        "{:.0},{:.0}",
        window.0 + window.2 + 2.0,
        window.1 + window.3 / 2.0
    );
    let wider = format!("{:.0},{:.0}", window.0 + 760.0, window.1 + window.3 / 2.0);
    app.run(&["drag", &edge, &wider]);
    app.snapshot("a_canvas_popped_out_into_a_window_hides_the_files_behind_it");
    app.close();
}
