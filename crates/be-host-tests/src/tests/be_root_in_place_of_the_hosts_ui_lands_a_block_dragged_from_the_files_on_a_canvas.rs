use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_lands_a_block_dragged_from_the_files_on_a_canvas() {
    // be-host's own UI makes a checklist and a canvas; started again with the root in place of
    // its UI, the checklist's row dragged from the Files onto the canvas lands on it: the files
    // app tells the root it drags the block, and the root carries it over the canvas, which
    // hears it there through its host and takes it when dropped.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Checklist");
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    let canvas = host.place("#infinite-canvas.canvas");
    let row = host.place("TreeItem \"Checklist\"");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    let start = (row.0 + row.2 / 2.0, row.1 + row.3 / 2.0);
    let to = (canvas.0 + canvas.2 / 2.0, canvas.1 + canvas.3 / 2.0);
    // The files app tells the root it drags the row once its drag starts, which the root lifts
    // at its next input; the pointer moves on only after that, as be-host's own test of this
    // waits for it.
    host.run(&["down", &format!("{},{}", start.0, start.1)]);
    host.run(&["move", &format!("{},{}", start.0 + 12.0, start.1)]);
    host.run(&["pause", "300"]);
    for step in 1..=4 {
        let share = step as f32 / 4.0;
        let x = start.0 + (to.0 - start.0) * share;
        let y = start.1 + (to.1 - start.1) * share;
        host.run(&["move", &format!("{x},{y}")]);
        host.run(&["pause", "100"]);
    }
    host.run(&["up"]);
    host.run(&["settle"]);
    host.snapshot("be_root_lands_a_block_dragged_from_the_files_on_a_canvas");
    host.close();
}
