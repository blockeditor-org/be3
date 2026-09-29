use block_editor_beui::be_block::map::MapRegion;
use block_editor_beui::be_block::{Map, MapContent};
use block_editor_beui::{Editor, EditorHost, FetchResult, HostReply, HostRequest};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::MapApp;

mod a_new_map_shows_the_whole_world;
mod labels_hold_still_between_frames;
mod labels_stay_while_deeper_tiles_load;
mod overlapping_labels_keep_the_larger_one;
mod reloading_fetches_the_tiles_again;
mod the_sidebar_captures_the_preview_region;
mod the_whole_world_button_redraws_in_the_run_it_is_clicked;
mod tiles_from_the_tile_source_are_drawn;
mod zooming_in_draws_deeper_tiles_over_the_shallower_ones;

fn editor() -> BeuiTest<MapApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor).in_viewport();
    editor.hold(None, MapContent::default());
    editor.run();
    editor
}

fn map(editor: &BeuiTest<MapApp>) -> Map {
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

fn labelled_tile(labels: &[(&str, &str, i64, i64)]) -> Vec<u8> {
    let mut layer = length_delimited(1, b"place_labels");
    layer.extend(varint(5 << 3));
    layer.extend(varint(EXTENT as u64));
    layer.extend(length_delimited(3, b"name"));
    layer.extend(length_delimited(3, b"kind"));
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
    let mut feature = varint(3 << 3);
    feature.extend(varint(3));
    feature.extend(length_delimited(4, &geometry));
    let mut layer = length_delimited(1, b"ocean");
    layer.extend(varint(5 << 3));
    layer.extend(varint(extent as u64));
    layer.extend(length_delimited(2, &feature));
    length_delimited(3, &layer)
}

fn tile_test_id(url: &str) -> String {
    let path = url
        .trim_end_matches(".mvt")
        .rsplitn(4, '/')
        .take(3)
        .collect::<Vec<_>>();
    format!("map.tile.{}.{}.{}", path[2], path[1], path[0])
}

fn tile_drawn(editor: &BeuiTest<MapApp>, test_id: &str) -> bool {
    let document = editor.document();
    document
        .find_test_id(test_id)
        .and_then(|picture| document.node_detail(picture))
        .is_some()
}

fn serve_tiles(editor: &mut BeuiTest<MapApp>) -> Vec<String> {
    serve_tiles_with(editor, ocean_tile)
}

fn requested_tiles(editor: &mut BeuiTest<MapApp>) -> Vec<(u64, String)> {
    editor
        .take_requests()
        .into_iter()
        .filter_map(|(request, asked)| match asked {
            HostRequest::Fetch(url) => Some((request, url)),
            _ => None,
        })
        .collect()
}

fn serve_tiles_with(editor: &mut BeuiTest<MapApp>, tile: impl Fn() -> Vec<u8>) -> Vec<String> {
    let mut served = Vec::new();
    loop {
        for (request, asked) in editor.take_requests() {
            let HostRequest::Fetch(url) = asked else {
                continue;
            };
            assert!(url.starts_with("https://vector.openstreetmap.org/") && url.ends_with(".mvt"));
            editor.reply(request, HostReply::Fetched(FetchResult::Body(tile())));
            served.push(url);
        }
        let ids: Vec<String> = served.iter().map(|url| tile_test_id(url)).collect();
        editor.settle_until("the served tiles to be drawn", |editor| {
            editor.has_requests() || ids.iter().all(|id| tile_drawn(editor, id))
        });
        if !editor.has_requests() {
            served.sort();
            return served;
        }
    }
}
