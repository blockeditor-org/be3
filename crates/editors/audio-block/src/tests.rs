use block_editor_beui::{AudioStatus, Editor, EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::{AudioApp, guess_media_type};
use audio_block_types::AudioFile;

mod a_playing_track_shows_its_position;
mod play_hands_the_host_the_tracks_bytes_once;
mod the_media_type_follows_the_file_name;
mod the_replace_panel_goes_away_with_the_chrome;

fn editor() -> (BeuiTest<AudioApp>, Editor) {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let owner = OwnedEditor::new(host.clone(), block);
    let editor = *owner;
    let mut test = BeuiTest::new(owner);
    test.hold(
        None,
        AudioFile::from_file("song.flac", "audio/flac", vec![1, 2, 3]).unwrap(),
    );
    test.run();
    (test, editor)
}
