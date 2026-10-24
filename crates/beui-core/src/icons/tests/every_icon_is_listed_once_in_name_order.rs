use super::*;

#[test]
fn every_icon_is_listed_once_in_name_order() {
    assert!(
        ALL.windows(2).all(|pair| pair[0].name < pair[1].name),
        "named looks icons up by binary search"
    );
    let mut glyphs: Vec<&str> = ALL.iter().map(|icon| icon.glyph).collect();
    glyphs.sort_unstable();
    glyphs.dedup();
    assert_eq!(glyphs.len(), ALL.len(), "no two icons share a codepoint");
    assert_eq!(named("copy-plus"), Some(ICON_COPY_PLUS));
    assert_eq!(named("not-an-icon"), None);
}
