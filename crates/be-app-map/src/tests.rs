use be_sdk_beui::beui::Vec2;
use be_sdk_beui::network::ResponseHead;
use be_sdk_test::prelude::*;
use be_sdk_test::{BeuiTest, Fetch};
use be_type_map::MapRegion;
use be_type_map::{Map, MapContent};
use uuid::Uuid;

use crate::root::MapRoot;

mod a_map_on_a_phone_on_its_side_shows_its_inspector_beside_it;
mod a_map_on_a_phone_shows_its_inspector_below_it;
mod a_new_map_shows_the_whole_world;
mod a_tile_source_naming_tiles_elsewhere_is_not_followed;
mod a_tile_source_that_cannot_be_read_says_why;
mod a_tile_the_rasterizer_fails_on_fails_alone;
mod a_town_is_drawn_from_its_openmaptiles_layers;
mod labels_hold_still_between_frames;
mod labels_stay_while_deeper_tiles_load;
mod overlapping_labels_keep_the_larger_one;
mod reloading_fetches_the_tiles_again;
mod the_sidebar_captures_the_preview_region;
mod the_whole_world_button_redraws_in_the_run_it_is_clicked;
mod tiles_from_the_tile_source_are_drawn;
mod zooming_in_draws_deeper_tiles_over_the_shallower_ones;

fn editor() -> BeuiTest<MapRoot> {
    sized(BeuiTest::editing(Uuid::new_v4()))
}

fn phone(size: Vec2) -> BeuiTest<MapRoot> {
    sized(BeuiTest::editing(Uuid::new_v4()).with_size(size))
}

fn sized(editor: BeuiTest<MapRoot>) -> BeuiTest<MapRoot> {
    holding(editor, MapContent::default())
}

/// `editor` in the camera the host gives the map, showing `content`.
fn holding(editor: BeuiTest<MapRoot>, content: MapContent) -> BeuiTest<MapRoot> {
    let mut editor = editor.in_viewport();
    editor.serve_worker::<be_worker_map_tiles::Tiles>(be_worker_map::MODULE);
    editor.store().register(be_type_map::registrations());
    editor.hold(None, content);
    editor.run();
    editor
}

fn map(editor: &BeuiTest<MapRoot>) -> Map {
    editor.content::<MapContent>(None).root()
}

fn varint(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            bytes.push(byte);
            return bytes;
        }
        bytes.push(byte | 0x80);
    }
}

fn length_delimited(field: u64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = varint(field << 3 | 2);
    bytes.extend(varint(payload.len() as u64));
    bytes.extend(payload);
    bytes
}

fn zigzag(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

const EXTENT: i64 = 4096;

fn ocean_tile() -> Vec<u8> {
    ocean_layer()
}

/// A tile of `water`, an ocean over all of it, and `place`, a point labelled `name` of `class` at
/// each of `labels`.
fn labelled_tile(labels: &[(&str, &str, i64, i64)]) -> Vec<u8> {
    let mut layer = length_delimited(1, b"place");
    layer.extend(varint(5 << 3));
    layer.extend(varint(EXTENT as u64));
    layer.extend(length_delimited(3, b"name"));
    layer.extend(length_delimited(3, b"class"));
    for (index, (name, kind, x, y)) in labels.iter().enumerate() {
        layer.extend(length_delimited(4, &length_delimited(1, name.as_bytes())));
        layer.extend(length_delimited(4, &length_delimited(1, kind.as_bytes())));
        let tags: Vec<u8> = [0, index as u64 * 2, 1, index as u64 * 2 + 1]
            .into_iter()
            .flat_map(varint)
            .collect();
        let geometry: Vec<u8> = [1 << 3 | 1, zigzag(*x), zigzag(*y)]
            .into_iter()
            .flat_map(varint)
            .collect();
        let mut feature = length_delimited(2, &tags);
        feature.extend(varint(3 << 3));
        feature.extend(varint(1));
        feature.extend(length_delimited(4, &geometry));
        layer.extend(length_delimited(2, &feature));
    }
    let mut tile = ocean_layer();
    tile.extend(length_delimited(3, &layer));
    tile
}

fn ocean_layer() -> Vec<u8> {
    let extent = EXTENT;
    let geometry: Vec<u8> = [
        1 << 3 | 1,
        zigzag(0),
        zigzag(0),
        3 << 3 | 2,
        zigzag(extent),
        zigzag(0),
        zigzag(0),
        zigzag(extent),
        zigzag(-extent),
        zigzag(0),
        1 << 3 | 7,
    ]
    .into_iter()
    .flat_map(varint)
    .collect();
    let mut feature = length_delimited(2, &[0, 0]);
    feature.extend(varint(3 << 3));
    feature.extend(varint(3));
    feature.extend(length_delimited(4, &geometry));
    let mut layer = length_delimited(1, b"water");
    layer.extend(length_delimited(2, &feature));
    layer.extend(length_delimited(3, b"class"));
    layer.extend(length_delimited(4, &length_delimited(1, b"ocean")));
    layer.extend(varint(5 << 3));
    layer.extend(varint(extent as u64));
    length_delimited(3, &layer)
}

/// Where the test's tile source keeps its tiles, as its TileJSON says.
const TILES: &str = "https://tiles.openfreemap.org/planet/20261115_080022_pt";

/// The tile source's TileJSON, trimmed to what the map reads.
fn tile_json() -> Vec<u8> {
    tile_json_naming(&format!("{TILES}/{{z}}/{{x}}/{{y}}.pbf"))
}

/// A TileJSON naming `template` for its tiles.
fn tile_json_naming(template: &str) -> Vec<u8> {
    format!(r#"{{"tilejson":"3.0.0","tiles":["{template}"],"maxzoom":14}}"#).into_bytes()
}

fn tile_test_id(url: &str) -> String {
    let path = url
        .trim_end_matches(".pbf")
        .rsplitn(4, '/')
        .take(3)
        .collect::<Vec<_>>();
    format!("map.tile.{}.{}.{}", path[2], path[1], path[0])
}

fn tile_drawn(editor: &BeuiTest<MapRoot>, test_id: &str) -> bool {
    let document = editor.document();
    document
        .find_test_id(test_id)
        .and_then(|picture| document.node_detail(picture))
        .is_some()
}

fn serve_tiles(editor: &mut BeuiTest<MapRoot>) -> Vec<String> {
    serve_tiles_with(editor, ocean_tile)
}

/// The tiles the map asks for. If it asks for the tile source's TileJSON first, this answers it
/// and returns the tiles it then asks for.
fn requested_tiles(editor: &mut BeuiTest<MapRoot>) -> Vec<(Fetch, String)> {
    let mut tiles = Vec::new();
    for fetch in editor.take_fetches() {
        assert_eq!(fetch.request.method, "GET");
        if fetch.request.url == crate::tiles::TILE_JSON {
            serve(editor, &fetch, tile_json());
            editor.settle_until("the map to ask for tiles", |editor| editor.has_fetches());
            tiles.extend(requested_tiles(editor));
            continue;
        }
        let url = fetch.request.url.clone();
        assert!(url.starts_with(TILES) && url.ends_with(".pbf"), "{url}");
        tiles.push((fetch, url));
    }
    tiles
}

/// Answers `fetch` with `tile`, its body in two chunks as a network delivers it.
fn serve(editor: &mut BeuiTest<MapRoot>, fetch: &Fetch, tile: Vec<u8>) {
    let (first, second) = tile.split_at(tile.len() / 2);
    let head = ResponseHead {
        status: 200,
        headers: Vec::new(),
    };
    editor.respond(fetch, head, [first.to_vec(), second.to_vec()]);
}

fn serve_tiles_with(editor: &mut BeuiTest<MapRoot>, tile: impl Fn() -> Vec<u8>) -> Vec<String> {
    serve_tiles_with_each(editor, |_| tile())
}

/// Serves each tile the map asks for with `tile` of its URL until it asks for no more, and waits
/// for them to be drawn; returns their URLs, sorted.
fn serve_tiles_with_each(
    editor: &mut BeuiTest<MapRoot>,
    tile: impl Fn(&str) -> Vec<u8>,
) -> Vec<String> {
    let mut served = Vec::new();
    loop {
        for (fetch, url) in requested_tiles(editor) {
            serve(editor, &fetch, tile(&url));
            served.push(url);
        }
        let ids: Vec<String> = served.iter().map(|url| tile_test_id(url)).collect();
        editor.settle_until("the served tiles to be drawn", |editor| {
            editor.has_fetches() || ids.iter().all(|id| tile_drawn(editor, id))
        });
        if !editor.has_fetches() {
            served.sort();
            return served;
        }
    }
}
