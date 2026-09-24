use std::sync::Arc;

use block_client::BlockClient;
use block_client::blocks::map::{Map as MapBlock, MapRegion};
use block_editor_plugin::be_block::{Map, MapContent};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::MapApp;

mod a_new_map_shows_the_whole_world;
mod the_sidebar_captures_the_preview_region;

fn editor() -> ContentHarness<MapApp> {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(MapBlock::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), client, block.id());
    let mut editor = ContentHarness::new(BeuiTest::new(editor).in_viewport(), host);
    editor.hold(None, MapContent::default());
    editor.run();
    editor.run();
    editor
}

fn map(editor: &ContentHarness<MapApp>) -> Map {
    editor.content::<MapContent>(None).root()
}
