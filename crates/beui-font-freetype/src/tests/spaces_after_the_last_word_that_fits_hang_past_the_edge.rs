use super::*;

#[test]
fn spaces_after_the_last_word_that_fits_hang_past_the_edge() {
    let mut fonts = fonts();
    let first = "abcd wordwordwordwordword ";
    let inked = width_of(&mut fonts, first.trim_end());
    assert!(
        inked - width_of(&mut fonts, "abcd ") > zone(),
        "the space after \"abcd\" lies before the zone"
    );
    let width = inked + 0.5;
    assert!(
        width < width_of(&mut fonts, first),
        "the space does not fit"
    );

    let lines = lines(&mut fonts, &format!("{first}next words"), width);
    assert_eq!(
        lines[0], first,
        "the line ends after the space, which hangs past the edge"
    );
    assert!(
        !lines[1].starts_with(' '),
        "the next line starts with its word, not a space: {lines:?}"
    );
}
