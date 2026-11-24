use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_a_picker_a_canvas_places() {
    // be-host's own UI opens a canvas; started again with the root in place of its UI, the
    // canvas's Block button opens its block picker: a popover the canvas anchors outside its
    // groups (`anchored-placement`), which the root shows over everything at the rect the canvas
    // gave it, with the picker the popover places in it, a dialog child the root opens in an app
    // of its own and sizes to the room it used. A press outside closes it, as under be-host.
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
    host.run(&["click", "600,300"]);
    host.run(&["settle"]);
    host.snapshot("be_root_closes_a_picker_a_press_outside_dismisses");
    host.close();
}
