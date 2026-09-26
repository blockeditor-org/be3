use block_editor_beui::be_block::{BlockContent, ImageContent};

use block_editor_beui::be_block::PixelArtContent;
use block_editor_beui::be_block::pixel_art::Artwork;
use block_editor_beui::be_block::pixel_art::PixelColor;
use block_editor_beui::beui::styled::toggle_button_pressed;
use block_editor_beui::beui::{Key, Pos2};
use block_editor_beui::{Artifact, Artifacts, Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::PixelArtApp;
use crate::artifact;

mod choosing_a_tool_marks_it_in_the_sidebar;
mod clearing_the_artwork_needs_the_dialog;
mod drawing_on_the_canvas_paints_the_block;
mod the_export_scale_setting_writes_the_artifact_draft;

fn editor() -> ContentHarness<PixelArtApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = ContentHarness::new(BeuiTest::new(editor), host);
    editor.hold(None, PixelArtContent::default());
    editor.run();
    editor.run();
    editor
}

fn art_of(editor: &ContentHarness<PixelArtApp>) -> Artwork {
    editor.content::<PixelArtContent>(None).root().artwork()
}
