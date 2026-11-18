use super::*;

/// When the tile source's TileJSON cannot be read, the map fetches no tiles and says why;
/// reloading reads it again.
#[test]
fn a_tile_source_that_cannot_be_read_says_why() {
    let mut editor = editor();
    let [fetch] = <[Fetch; 1]>::try_from(editor.take_fetches()).expect("one fetch, the TileJSON");
    assert_eq!(fetch.request.url, crate::tiles::TILE_JSON);

    let head = ResponseHead {
        status: 503,
        headers: Vec::new(),
    };
    editor.respond(&fetch, head, [Vec::new()]);
    editor.settle_until("the map to say why", |editor| editor.shown("map.error"));

    assert!(!editor.has_fetches());
    assert!(
        editor
            .label("map.error")
            .starts_with("the tile source could not be read"),
        "{}",
        editor.label("map.error")
    );

    editor.click("map.reload");
    assert_eq!(serve_tiles(&mut editor).len(), 16);
}
