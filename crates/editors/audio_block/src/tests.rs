use block_editor_beui::be_block::AudioContent;
use block_editor_beui::{AudioStatus, Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::{AudioApp, guess_media_type};

mod a_playing_track_shows_its_position;
mod the_media_type_follows_the_file_name;
mod the_replace_panel_goes_away_with_the_chrome;

fn editor() -> (ContentHarness<AudioApp>, Editor) {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut test = ContentHarness::new(BeuiTest::new(editor.clone()), host);
    test.hold(
        None,
        AudioContent::from_file("song.flac", "audio/flac", vec![1, 2, 3]).unwrap(),
    );
    test.run();
    (test, editor)
}
