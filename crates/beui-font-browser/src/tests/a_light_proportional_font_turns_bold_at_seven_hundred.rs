use super::*;

#[test]
fn a_light_proportional_font_turns_bold_at_seven_hundred() {
    let font = FontId::proportional(14.0);
    assert!(css_font(font).starts_with("normal 300 14px Ubuntu,"));
    assert!(css_font(font.bold(true).italic(true)).starts_with("italic 700 14px Ubuntu,"));
    assert_eq!(css_font(FontId::icons(24.0)), "normal 400 24px beui-icons");
}
