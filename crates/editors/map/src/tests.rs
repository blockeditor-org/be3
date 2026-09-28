use block_editor_beui::be_block::map::MapRegion;
use block_editor_beui::be_block::{Map, MapContent};
use block_editor_beui::{Editor, EditorHost, FetchResult, HostReply, HostRequest};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::MapApp;

mod a_new_map_shows_the_whole_world;
mod reloading_fetches_the_tiles_again;
mod the_sidebar_captures_the_preview_region;
mod tiles_from_the_tile_source_are_drawn;

fn editor() -> BeuiTest<MapApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor).in_viewport();
    editor.hold(None, MapContent::default());
    editor.run();
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

fn ocean_tile() -> Vec<u8> {
    let extent = 4096;
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
    let mut served = Vec::new();
    loop {
        for (request, asked) in editor.take_requests() {
            let HostRequest::Fetch(url) = asked else {
                continue;
            };
            assert!(url.starts_with("https://vector.openstreetmap.org/") && url.ends_with(".mvt"));
            editor.reply(request, HostReply::Fetched(FetchResult::Body(ocean_tile())));
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
