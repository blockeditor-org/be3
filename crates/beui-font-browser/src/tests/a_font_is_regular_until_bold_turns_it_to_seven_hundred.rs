use super::*;

#[test]
fn a_font_is_regular_until_bold_turns_it_to_seven_hundred() {
    let font = FontId::proportional(14.0);
    assert!(css_font(font).starts_with("normal 400 14px Inter,"));
    assert!(css_font(font.bold(true).italic(true)).starts_with("italic 700 14px Inter,"));
    assert_eq!(css_font(FontId::icons(24.0)), "normal 400 24px beui-icons");
}
