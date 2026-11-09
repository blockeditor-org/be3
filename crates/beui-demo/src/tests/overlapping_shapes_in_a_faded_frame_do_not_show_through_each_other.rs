use super::*;

#[test]
fn overlapping_shapes_in_a_faded_frame_do_not_show_through_each_other() {
    let mut test = alone(WIDE, Page::Layering);
    reveal(&mut test, "demo.layering.faded_overlap");
    test.snapshot("overlapping_shapes_in_a_faded_frame_do_not_show_through_each_other");
}
