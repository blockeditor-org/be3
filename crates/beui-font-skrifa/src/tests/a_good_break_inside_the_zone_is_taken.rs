use super::*;

#[test]
fn a_good_break_inside_the_zone_is_taken() {
    let mut fonts = fonts();
    let words = "aaaa ".repeat(4);
    let before = width_of(&mut fonts, &words);
    let width = before + 50.0;
    assert!(50.0 < zone(), "the space lies inside the break zone");
    let text = format!("{words}{}", "b".repeat(40));

    let lines = lines(&mut fonts, &text, width);
    assert_eq!(
        lines[0], words,
        "the line breaks at the space 50 px before the edge rather than inside the b's"
    );
}
