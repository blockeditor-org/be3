use super::*;

#[test]
fn text_in_a_drawing_lands_where_a_text_node_puts_it() {
    let mut test = alone(WIDE, Page::TextNodes);
    reveal(&mut test, "demo.text.drawn");
    test.snapshot("text_in_a_drawing_lands_where_a_text_node_puts_it");
}
