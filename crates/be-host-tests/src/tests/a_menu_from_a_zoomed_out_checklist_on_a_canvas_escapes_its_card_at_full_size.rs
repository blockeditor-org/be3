use super::*;

const PANE: &str = "Pane \"Checklist\"";
const ROW: &str = "MenuItem \"Select All\"";

#[test]
fn a_menu_from_a_zoomed_out_checklist_on_a_canvas_escapes_its_card_at_full_size() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    add_to_canvas(&mut app, "Checklist");
    app.wait("#checklist.draft");
    app.run(&["click", "#checklist.draft", "right"]);
    app.wait(ROW);
    let (_, _, full_width, full_height) = app.place(ROW);
    app.key("Escape");
    app.gone(ROW);

    app.click("#infinite-canvas.pill.zoom-out");
    app.click("#infinite-canvas.pill.zoom-out");
    let (pane_x, pane_y, pane_width, pane_height) = app.place(PANE);
    let (_, _, draft_width, _) = app.place("#checklist.draft");
    app.run(&["click", "#checklist.draft", "right"]);
    app.wait(ROW);
    let (row_x, row_y, row_width, row_height) = app.place(ROW);
    assert!(
        (row_width - full_width).abs() <= 1.5 && (row_height - full_height).abs() <= 1.5,
        "the menu of a zoomed-out checklist is full size: {row_width}x{row_height}, not {full_width}x{full_height}, while its field is {draft_width} wide"
    );
    assert!(
        row_x + row_width > pane_x + pane_width || row_y + row_height > pane_y + pane_height,
        "and reaches past the checklist's card"
    );
    app.crop("#infinite-canvas.canvas");
    app.snapshot("a_menu_from_a_zoomed_out_checklist_on_a_canvas_escapes_its_card_at_full_size");
    app.click(ROW);
    app.gone(ROW);
    app.close();
}
