use block_editor_beui::be_block::BlockContent;
use image_block_types::ImageContent;

use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Key, Pos2};
use block_editor_beui::{Artifact, Artifacts, EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use pixel_art_types::Artwork;
use pixel_art_types::PixelArtContent;
use pixel_art_types::PixelColor;
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
    let editor = OwnedEditor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor);
    editor.store().register(pixel_art_types::registrations());
    editor.hold(None, PixelArtContent::default());
    editor.run();
    editor
}

fn art_of(editor: &BeuiTest<PixelArtApp>) -> Artwork {
    editor.content::<PixelArtContent>(None).root().artwork()
}
