use super::*;

#[test]
fn an_edit_to_the_block_shows_in_its_debug_view() {
    let mut test = editor();
    test.edit::<CounterContent>(None, &Counter::add(4));
    test.run();
    assert!(says(&test, "count 7"), "the edit reached the view");
}
