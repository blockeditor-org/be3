use super::*;

#[test]
fn a_map_on_a_phone_on_its_side_shows_its_inspector_beside_it() {
    let mut editor = phone(Vec2::new(844.0, 390.0));
    serve_tiles(&mut editor);
    editor.settle();

    assert!(
        editor.shown("map.toolbar") && editor.shown("map.preview-region"),
        "a phone shows the map with its toolbar and the inspector gathered beside it"
    );
    editor.snapshot("a_map_on_a_phone_on_its_side_shows_its_inspector_beside_it");
}
