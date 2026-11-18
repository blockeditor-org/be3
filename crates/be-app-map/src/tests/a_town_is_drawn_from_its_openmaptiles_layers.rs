use super::*;

/// OpenFreeMap's tile 14/8774/5741, Zell am See, as it served it in November 2026, gzipped: a lake,
/// rivers, woods and farmland, a town of roads and buildings, a railway and place names.
const ZELL_AM_SEE: &[u8] = include_bytes!("tiles/zell_am_see_14_8774_5741.pbf.gz");

/// A real tile of the OpenMapTiles schema draws its water, land cover and use, roads by class,
/// railway, buildings and place labels. The test's viewport zooms no further than 32 times, so
/// the map zooms in that far and is served the town for every tile it asks for.
#[test]
fn a_town_is_drawn_from_its_openmaptiles_layers() {
    let mut editor = editor();
    serve_tiles(&mut editor);
    for _ in 0..5 {
        editor.click("map.zoom-in");
    }

    let served = serve_tiles_with(&mut editor, || ZELL_AM_SEE.to_vec());

    assert!(!served.is_empty());
    assert!((1..=16).any(|occurrence| editor.shown(&format!("map.label.Seespitz.{occurrence}"))));
    editor.snapshot("a_town_is_drawn_from_its_openmaptiles_layers");
}
