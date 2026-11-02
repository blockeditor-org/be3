use super::*;

#[test]
fn tiles_from_the_tile_source_are_drawn() {
    let mut editor = editor();

    let served = serve_tiles(&mut editor);

    assert_eq!(served.len(), 16);
    assert!(served.iter().all(|url| url.contains("/shortbread_v1/2/")));
    editor.snapshot("tiles_from_the_tile_source_are_drawn");
}
