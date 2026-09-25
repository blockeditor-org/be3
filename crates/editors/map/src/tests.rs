use block_editor_beui::be_block::map::MapRegion;
use block_editor_beui::be_block::{Map, MapContent};
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::MapApp;

mod a_new_map_shows_the_whole_world;
mod the_sidebar_captures_the_preview_region;

fn editor() -> ContentHarness<MapApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = ContentHarness::new(BeuiTest::new(editor).in_viewport(), host);
    editor.hold(None, MapContent::default());
    editor.run();
    editor.run();
    editor
}

fn map(editor: &ContentHarness<MapApp>) -> Map {
    editor.content::<MapContent>(None).root()
}
