use be_sdk_beui::{AudioStatus, Editor, EditorHost, OwnedEditor};
use be_sdk_test::BeuiTest;
use be_sdk_test::prelude::*;
use uuid::Uuid;

use crate::root::AudioRoot;
use be_type_audio::{AudioFile, BlobFile};

mod a_playing_track_shows_its_position;
mod a_track_on_a_phone_on_its_side_shows_its_panel_beside_it;
mod a_track_on_a_phone_shows_its_panel_below_it;
mod play_hands_the_host_the_tracks_bytes_once;
mod the_replace_panel_goes_away_with_the_chrome;

fn editor() -> (BeuiTest<AudioRoot>, Editor) {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let owner = OwnedEditor::new(host.clone(), block);
    let editor = *owner;
    let mut test = BeuiTest::new(owner);
    test.hold(
        None,
        AudioFile::from_file("song.flac", vec![1, 2, 3]),
    );
    test.run();
    (test, editor)
}
