use super::*;

#[test]
fn a_word_too_long_for_any_line_never_widens_the_text() {
    let mut fonts = fonts();
    let width = 300.0;
    let text = format!("abcd {} {}", "e".repeat(150), "abcd ".repeat(30));
    let galley = fonts.layout(&text, font(), TextLayout::wrapped(width), 1.0);
    assert!(
        galley.size().x <= width,
        "the text is no wider than it was given: {}",
        galley.size().x
    );
    for line in galley.lines() {
        let shown = &galley.text()[line.range.clone()];
        assert!(
            width_of(&mut fonts, shown.trim_end()) <= width,
            "{shown:?} runs past {width}"
        );
    }
    let short = galley
        .lines()
        .iter()
        .map(|line| &galley.text()[line.range.clone()])
        .filter(|line| line.starts_with("abcd abcd"))
        .collect::<Vec<_>>();
    assert!(
        short.len() > 1,
        "the short words after the long one wrap at the width too"
    );
}
