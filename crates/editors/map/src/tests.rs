use std::sync::Arc;

use block_client::blocks::map::{Map, MapRegion};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::MapApp;

mod a_new_map_shows_the_whole_world;
mod the_sidebar_captures_the_preview_region;

fn editor() -> (BeuiTest<MapApp>, BlockHandle<Map>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Map::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::new(editor).in_viewport();
    editor.run();
    editor.run();
    (editor, block)
}

fn displayed_region(block: &BlockHandle<Map>) -> MapRegion {
    block
        .read()
        .map_or(MapRegion::WORLD, |map| map.displayed_region())
}
