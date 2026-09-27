use super::*;

#[test]
fn hex_accepts_the_short_and_long_forms_with_or_without_alpha() {
    assert_eq!(parse_hex("#f80"), Some(Color32::from_rgb(0xFF, 0x88, 0x00)));
    assert_eq!(
        parse_hex("f808"),
        Some(Color32::from_rgba_unmultiplied(0xFF, 0x88, 0x00, 0x88))
    );
    assert_eq!(parse_hex(" #102030 "), Some(Color32::from_rgb(0x10, 0x20, 0x30)));
    assert_eq!(
        parse_hex("#10203040"),
        Some(Color32::from_rgba_unmultiplied(0x10, 0x20, 0x30, 0x40))
    );
    assert_eq!(parse_hex("#12345"), None);
    assert_eq!(parse_hex("#zzzzzz"), None);
    assert_eq!(
        format_hex(Color32::from_rgba_unmultiplied(1, 2, 3, 4), true),
        "#01020304"
    );
    assert_eq!(format_hex(Color32::from_rgb(1, 2, 3), false), "#010203");
}
