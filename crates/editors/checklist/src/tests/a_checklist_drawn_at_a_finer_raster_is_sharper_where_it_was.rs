use super::*;

#[test]
fn a_checklist_drawn_at_a_finer_raster_is_sharper_where_it_was() {
    let mut checklist = Harness::new(&[("buy milk", false), ("call the vet", true)]);
    checklist.place(block_editor_beui::beui::vec2(440.0, 340.0), 3.0);
    checklist.snapshot("a_checklist_drawn_at_a_finer_raster_is_sharper_where_it_was");
}
