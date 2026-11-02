use super::*;

const PANE: &str = "Pane \"Checklist\"";
const ROW: &str = "MenuItem \"Select All\"";

#[test]
fn a_menu_from_a_zoomed_out_checklist_on_a_canvas_escapes_its_card_at_full_size() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Checklist");
    host.wait("#checklist.draft");
    host.run(&["click", "#checklist.draft", "right"]);
    host.wait(ROW);
    let (_, _, full_width, full_height) = host.place(ROW);
    host.key("Escape");
    host.gone(ROW);

    host.click("#infinite-canvas.pill.zoom-out");
    host.click("#infinite-canvas.pill.zoom-out");
    let (pane_x, pane_y, pane_width, pane_height) = host.place(PANE);
    let (_, _, draft_width, _) = host.place("#checklist.draft");
    host.run(&["click", "#checklist.draft", "right"]);
    host.wait(ROW);
    let (row_x, row_y, row_width, row_height) = host.place(ROW);
    assert!(
        (row_width - full_width).abs() <= 1.5 && (row_height - full_height).abs() <= 1.5,
        "the menu of a zoomed-out checklist is full size: {row_width}x{row_height}, not {full_width}x{full_height}, while its field is {draft_width} wide"
    );
    assert!(
        row_x + row_width > pane_x + pane_width || row_y + row_height > pane_y + pane_height,
        "and reaches past the checklist's card"
    );
    host.crop("#infinite-canvas.canvas");
    host.snapshot("a_menu_from_a_zoomed_out_checklist_on_a_canvas_escapes_its_card_at_full_size");
    host.click(ROW);
    host.gone(ROW);
    host.close();
}
