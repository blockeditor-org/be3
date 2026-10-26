use crate::strip_comments as strip;

#[test]
fn strip_comments_keeps_every_comment_in_a_file_marked_to_keep_them() {
    let source = br#"// Generated from the icon font. @yeskeepfile
pub const ICON_X: &str = "x"; // tags: close, cancel
pub const ICON_Y: &str = "y"; // tags: letter
"#;

    assert_eq!(strip(source).unwrap(), source.to_vec());
    assert_eq!(
        String::from_utf8(strip(&source[46..]).unwrap()).unwrap(),
        "pub const ICON_X: &str = \"x\";\npub const ICON_Y: &str = \"y\";\n"
    );
}
