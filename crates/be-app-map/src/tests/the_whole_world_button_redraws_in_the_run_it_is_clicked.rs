use super::*;

#[test]
fn the_whole_world_button_redraws_in_the_run_it_is_clicked() {
    let mut editor = editor();
    let world = serve_tiles(&mut editor);
    for _ in 0..5 {
        editor.click("map.zoom-in");
        editor.run();
    }
    assert!(!tile_drawn(&editor, &tile_test_id(&world[0])));

    editor.click("map.fit");
    editor.run();

    assert!(
        world
            .iter()
            .all(|url| tile_drawn(&editor, &tile_test_id(url)))
    );
}
