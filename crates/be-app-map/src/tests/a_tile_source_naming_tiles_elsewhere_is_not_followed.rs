use super::*;

/// A TileJSON naming tiles on another host, or without `{z}`, `{x}` and `{y}`, fetches no tiles:
/// the map says why.
#[test]
fn a_tile_source_naming_tiles_elsewhere_is_not_followed() {
    for template in [
        "https://example.com/{z}/{x}/{y}.pbf",
        "https://tiles.openfreemap.org.example.com/{z}/{x}/{y}.pbf",
        "https://tiles.openfreemap.org/planet/{z}/{x}.pbf",
    ] {
        let mut editor = editor();
        let [fetch] =
            <[Fetch; 1]>::try_from(editor.take_fetches()).expect("one fetch, the TileJSON");
        assert_eq!(fetch.request.url, crate::tiles::TILE_JSON);

        serve(&mut editor, &fetch, tile_json_naming(template));
        editor.settle_until("the map to say why", |editor| editor.shown("map.error"));

        assert!(!editor.has_fetches(), "{template}");
        assert_eq!(
            editor.label("map.error"),
            "the tile source names tiles elsewhere"
        );
    }
}
