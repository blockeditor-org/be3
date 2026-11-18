use super::*;

#[test]
fn a_block_dragged_from_the_files_onto_a_canvas_lands_on_it() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Checklist");
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    let canvas = host.place("#infinite-canvas.canvas");
    let middle = format!(
        "{:.0},{:.0}",
        canvas.0 + canvas.2 / 2.0,
        canvas.1 + canvas.3 / 2.0
    );
    host.run(&["drag", "TreeItem \"Checklist\"", &middle, "8"]);
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
