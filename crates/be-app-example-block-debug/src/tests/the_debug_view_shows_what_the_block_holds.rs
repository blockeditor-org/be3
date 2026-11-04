use super::*;

#[test]
fn the_debug_view_shows_what_the_block_holds() {
    let mut test = editor();
    assert!(says(&test, "content"), "the content is listed");
    assert!(says(&test, "count 3"), "the counter's value is shown");
    test.snapshot("the_debug_view_shows_what_the_block_holds");
}
