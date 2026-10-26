use super::*;

#[test]
fn text_breaks_end_lines_and_breaking_anywhere_fills_them() {
    let mut test = alone(WIDE, Page::TextNodes);
    reveal(&mut test, "demo.text.breaks");
    test.snapshot("text_breaks_end_lines_and_breaking_anywhere_fills_them");
}
