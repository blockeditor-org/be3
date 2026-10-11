use super::*;

#[test]
fn a_cursor_the_theme_lacks_falls_back_to_its_default_then_to_the_arrow() {
    let icons = Icons::new("fallback");
    icons.cursor("default", "xterm", &[drawn(24, (12, 12), 0xffff_ffff)]);
    icons.cursor("default", "left_ptr", &[drawn(24, (2, 3), 0xff00_0000)]);
    let theme = icons.theme("default", 24);

    let Choice::Themed { hotspot, .. } = choose(&theme, CursorIcon::Text, 1.0) else {
        panic!("the text cursor is found by its X11 name");
    };
    assert_eq!(hotspot, vec2(12.0, 12.0));
    let Choice::Themed { hotspot, .. } = choose(&theme, CursorIcon::ZoomIn, 1.0) else {
        panic!("a cursor the theme lacks shows the theme's default");
    };
    assert_eq!(hotspot, vec2(2.0, 3.0));
    assert_eq!(choose(&theme, CursorIcon::None, 1.0), Choice::Hidden);

    let empty = Icons::new("empty");
    assert_eq!(
        choose(&empty.theme("default", 24), CursorIcon::Text, 1.0),
        Choice::Arrow,
        "without a theme the built-in arrow is shown"
    );
    assert_eq!(
        choose(&empty.theme("default", 24), CursorIcon::None, 1.0),
        Choice::Hidden
    );
}
