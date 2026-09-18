use std::sync::Arc;

use block_client::blocks::audio::Audio;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{AudioStatus, Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::{AudioApp, guess_media_type};

mod a_playing_track_shows_its_position;
mod the_media_type_follows_the_file_name;
mod the_replace_panel_goes_away_with_the_chrome;

fn editor() -> (BeuiTest<AudioApp>, Editor, BlockHandle<Audio>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Audio::new("song.flac", "audio/flac", vec![1, 2, 3]).unwrap());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut test = BeuiTest::new(editor.clone());
    test.run();
    (test, editor, block)
}
