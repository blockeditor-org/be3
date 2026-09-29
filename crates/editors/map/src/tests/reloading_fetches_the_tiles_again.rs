use super::*;

#[test]
fn reloading_fetches_the_tiles_again() {
    let mut editor = editor();
    let served = serve_tiles(&mut editor);

    editor.click("map.reload");
    editor.run();

    assert!(!tile_drawn(&editor, &tile_test_id(&served[0])));
    assert_eq!(serve_tiles(&mut editor), served);
}
