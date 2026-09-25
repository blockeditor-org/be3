use block_editor_plugin::be_block::{BlockContent, ImageContent};

use block_editor_plugin::be_block::PixelArtContent;
use block_editor_plugin::be_block::pixel_art::Artwork;
use block_editor_plugin::be_block::pixel_art::PixelColor;
use block_editor_plugin::beui::styled::toggle_button_pressed;
use block_editor_plugin::beui::{Key, Modifiers, Pos2};
use block_editor_plugin::{Artifact, Artifacts, Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::PixelArtApp;
use crate::artifact;

mod choosing_a_tool_marks_it_in_the_sidebar;
mod clearing_the_artwork_needs_the_dialog;
mod drawing_on_the_canvas_paints_the_block;
mod the_export_scale_setting_writes_the_artifact_draft;

fn editor() -> BeuiTest<PixelArtApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor);
    editor.hold(None, PixelArtContent::default());
    editor.run();
    editor.run();
    editor
}

fn art_of(editor: &BeuiTest<PixelArtApp>) -> Artwork {
    editor.content::<PixelArtContent>(None).root().artwork()
}
