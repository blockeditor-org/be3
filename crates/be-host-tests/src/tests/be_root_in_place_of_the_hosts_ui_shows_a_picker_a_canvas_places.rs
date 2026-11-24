use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_a_picker_a_canvas_places() {
    // be-host's own UI opens a canvas; started again with the root in place of its UI, the
    // canvas's Block button opens its block picker: a popover the canvas anchors outside its
    // groups (`anchored-placement`), which the root shows over everything at the rect the canvas
    // gave it, with the picker the popover places in it, a dialog child the root opens in an app
    // of its own and sizes to the room it used. The picker takes the keyboard, as under
    // be-host: Down moves its highlight. A press outside closes it; opened again, Escape closes
    // it, and the canvas has the keyboard back, on the Block button, where Enter opens it again.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Canvas");
    // The root lays the canvas out where be-host did.
    let block = host.place("#infinite-canvas.dock.block");
    let block = format!(
        "{:.0},{:.0}",
        block.0 + block.2 / 2.0,
        block.1 + block.3 / 2.0
    );
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.run(&["click", &block]);
    host.run(&["settle"]);
    host.snapshot("be_root_shows_a_picker_a_canvas_places");
    host.key("Down");
    host.run(&["settle"]);
    host.snapshot("be_root_hands_the_keys_to_a_picker_a_canvas_places");
    host.run(&["click", "600,300"]);
    host.run(&["settle"]);
    host.snapshot("be_root_closes_a_picker_a_press_outside_dismisses");
    host.run(&["click", &block]);
    host.run(&["settle"]);
    host.key("Escape");
    host.run(&["settle"]);
    host.key("Enter");
    host.run(&["settle"]);
    host.snapshot("be_root_gives_the_keys_back_to_the_canvas_a_picker_closed_and_enter_reopens_it");
    host.close();
}
