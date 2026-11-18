use super::*;

#[test]
fn a_block_dragged_from_the_files_onto_a_canvas_lands_on_it() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Checklist");
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    let canvas = host.place("#infinite-canvas.canvas");
    let row = host.place("TreeItem \"Checklist\"");
    let start = (row.0 + row.2 / 2.0, row.1 + row.3 / 2.0);
    let to = (canvas.0 + canvas.2 / 2.0, canvas.1 + canvas.3 / 2.0);
    // The files app tells the host it drags the row once its drag starts; the host lifts the
    // block then, so the pointer moves on only after that.
    host.run(&["down", "TreeItem \"Checklist\""]);
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
    host.wait("#checklist.add");
    let card = host.place("#checklist.add");
    assert!(
        card.0 > canvas.0
            && card.1 > canvas.1
            && card.0 + card.2 < canvas.0 + canvas.2
            && card.1 + card.3 < canvas.1 + canvas.3,
        "the block the host carried from the files lands on the canvas: {card:?} in {canvas:?}"
    );
    host.snapshot("a_block_dragged_from_the_files_onto_a_canvas_lands_on_it");
    host.close();
}
