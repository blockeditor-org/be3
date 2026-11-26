use std::sync::Arc;

use super::*;

const TEXT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
<text x="2" y="40" style="font-size:40px;font-family:'Font Nobody Has';fill:#000">X</text></svg>"##;

const CLEAR: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16">
<rect width="16" height="16" fill="none"/></svg>"##;

#[test]
fn an_svg_of_only_text_draws_its_letters() {
    let image = icons::render_svg(TEXT.as_bytes(), 48, || Arc::new(icons::bundled_fonts()))
        .expect("the svg renders");
    let inked = image
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] > 128)
        .count();
    assert!(inked > 50, "the letter is drawn, {inked} pixels are inked");

    let scratch = Scratch::new();
    let clear = scratch.write("clear.svg", CLEAR.as_bytes());
    assert!(
        load_icon(&clear, 16).is_none(),
        "an icon that draws nothing is no icon"
    );
}
