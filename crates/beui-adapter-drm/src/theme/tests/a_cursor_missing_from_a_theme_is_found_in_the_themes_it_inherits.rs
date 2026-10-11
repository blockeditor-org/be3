use super::*;

#[test]
fn a_cursor_missing_from_a_theme_is_found_in_the_themes_it_inherits() {
    let icons = Icons::new("inherits");
    icons.index(
        "mine",
        "[Icon Theme]\nName=Mine\nInherits=missing, parent\n",
    );
    icons.index("parent", "[Icon Theme]\nInherits=mine\n");
    icons.cursor("mine", "default", &[drawn(24, (1, 1), 0xff00_0000)]);
    icons.cursor("parent", "text", &[drawn(24, (12, 12), 0xffff_ffff)]);
    icons.cursor("default", "wait", &[drawn(24, (12, 12), 0xffff_ffff)]);
    let theme = icons.theme("mine", 24);

    assert_eq!(
        theme.find("default"),
        Some(icons.0.join("mine/cursors/default"))
    );
    assert_eq!(
        theme.find("text"),
        Some(icons.0.join("parent/cursors/text")),
        "an inherited theme is searched, past one that is missing"
    );
    assert_eq!(
        theme.find("wait"),
        Some(icons.0.join("default/cursors/wait")),
        "the default theme is searched last"
    );
    assert_eq!(
        theme.find("pointer"),
        None,
        "themes that inherit each other end the search"
    );
    assert_eq!(theme.find("../parent/cursors/text"), None);
}
