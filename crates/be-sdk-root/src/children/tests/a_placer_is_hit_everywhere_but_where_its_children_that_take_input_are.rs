use super::*;

#[test]
fn a_placer_is_hit_everywhere_but_where_its_children_that_take_input_are() {
    // With no child that takes input, the placer takes every press.
    assert_eq!(placer_hit((100, 80), Vec::new()), Area::Whole);
    assert!(takes_input(ChildMode::Live));
    assert!(takes_input(ChildMode::Active));
    assert!(takes_input(ChildMode::InPlace));
    assert!(!takes_input(ChildMode::Passive));
    assert!(!takes_input(ChildMode::Preview));
    assert!(runs_editor(ChildMode::Preview));
    assert!(!runs_editor(ChildMode::Hidden));

    // A live child at its left leaves the placer the rest.
    let Area::Region(rects) = placer_hit(
        (100, 80),
        vec![(rect((0.0, 0.0), (40.0, 80.0)), Vec::new())],
    ) else {
        panic!("the placer is hit only around its child");
    };
    assert_eq!(rects, vec![rect((40.0, 0.0), (100.0, 80.0))]);
}
