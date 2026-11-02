use super::*;

#[test]
fn a_canvas_popped_out_into_a_window_hides_the_files_behind_it() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Canvas");
    host.wait("Pane \"Canvas\"");
    host.run(&["click", "#dock.grip.editors", "right"]);
    host.run(&["click", "MenuItemCheckBox \"Locked\""]);
    host.run(&["click", "Tab \"Canvas\"", "right"]);
    host.run(&["click", "MenuItem \"Pop out into a window\""]);
    host.wait("Tab \"Canvas\"");
    let window = host.place("Pane \"Canvas\"");
    let edge = format!(
        "{:.0},{:.0}",
        window.0 + window.2 + 2.0,
        window.1 + window.3 / 2.0
    );
    let wider = format!("{:.0},{:.0}", window.0 + 760.0, window.1 + window.3 / 2.0);
    host.run(&["drag", &edge, &wider]);
    host.snapshot("a_canvas_popped_out_into_a_window_hides_the_files_behind_it");
    host.close();
}
