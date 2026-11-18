use super::*;

/// OpenFreeMap's tile 14/8774/5741, Zell am See, as it served it in November 2026, gzipped: a lake,
/// rivers, woods and meadows, a town of roads and buildings, a railway, a pier and place names.
/// © OpenMapTiles © OpenStreetMap contributors, ODbL 1.0 (openstreetmap.org/copyright), served by
/// OpenFreeMap.
const ZELL_AM_SEE: &[u8] = include_bytes!("tiles/zell_am_see_14_8774_5741.pbf.gz");

/// A real tile of the OpenMapTiles schema, shown at its own zoom in the middle of a region twice its
/// size, its neighbours empty, draws its water, land cover and
/// use, roads by class, railway, buildings and place labels.
#[test]
fn a_town_is_drawn_from_its_openmaptiles_layers() {
    let mut content = MapContent::default();
    content.apply(&Map::set_preview_region(Some(MapRegion {
        west: 12.777_100,
        south: 47.301_585,
        east: 12.821_045,
        north: 47.331_377,
    })));
    let mut editor = holding(BeuiTest::editing(Uuid::new_v4()), content);

    let served = serve_tiles_with_each(&mut editor, |url| {
        match url.ends_with("/14/8774/5741.pbf") {
            true => ZELL_AM_SEE.to_vec(),
            false => Vec::new(),
        }
    });

    assert!(served.iter().any(|url| url.ends_with("/14/8774/5741.pbf")));
    assert!(editor.shown("map.label.Seespitz.1"));
    editor.snapshot("a_town_is_drawn_from_its_openmaptiles_layers");
}
