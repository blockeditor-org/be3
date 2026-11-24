use super::*;

#[test]
fn a_long_word_after_a_short_one_breaks_at_the_edge_not_back_at_the_space() {
    let mut fonts = fonts();
    let width = 300.0;
    let text = format!("abcd {}", "e".repeat(60));
    assert!(
        width - width_of(&mut fonts, "abcd ") > zone(),
        "the space lies before the break zone"
    );
    let e = width_of(&mut fonts, "e");

    let lines = lines(&mut fonts, &text, width);
    let first = width_of(&mut fonts, &lines[0]);
    assert!(
        lines[0].starts_with("abcd e"),
        "the e's start on the first line rather than leaving \"abcd \" alone: {lines:?}"
    );
    assert!(
        first <= width && width - first < e,
        "the first line ends at the last e that fits, {first} of {width}: {lines:?}"
    );
}
