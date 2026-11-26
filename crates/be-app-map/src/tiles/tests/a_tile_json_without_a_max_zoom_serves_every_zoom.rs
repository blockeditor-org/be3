use super::*;

/// TileJSON 3.0 makes `maxzoom` optional, defaulting to 30: such a source serves every zoom the map
/// shows, so no tile is cut from another.
#[test]
fn a_tile_json_without_a_max_zoom_serves_every_zoom() {
    let tile_json: TileJson = serde_json::from_str(
        r#"{"tilejson":"3.0.0","tiles":["https://tiles.openfreemap.org/t/{z}/{x}/{y}.pbf"]}"#,
    )
    .expect("the TileJSON parses");

    let source = TileSource::from_tile_json(tile_json).expect("the tiles are OpenFreeMap's");

    assert_eq!(source.max_zoom, 30);
    let deepest = TileId {
        zoom: MAX_TILE_ZOOM,
        x: 5,
        y: 9,
    };
    assert_eq!(deepest.source(source.max_zoom), deepest);
}
