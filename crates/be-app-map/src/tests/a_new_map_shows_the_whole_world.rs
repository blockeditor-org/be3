use super::*;

#[test]
fn a_new_map_shows_the_whole_world() {
    let mut editor = editor();

    assert_eq!(map(&editor).preview_region, None);
    assert_eq!(map(&editor).displayed_region(), MapRegion::WORLD);
    editor.snapshot("a_new_map_shows_the_whole_world");
}
