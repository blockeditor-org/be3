use std::sync::Arc;

use block_client::block_ref::BlockRef;
use block_client::blocks::counter::Counter;
use block_client::blocks::video::{Video, VideoClip, VideoFrameRate, VideoOperation};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::VideoApp;
use crate::timeline::timecode;

mod an_empty_video_has_nothing_at_the_playhead;
mod clicking_a_clip_selects_it_for_the_effects_panel;
mod timecode_counts_minutes_seconds_and_frames;

fn editor() -> (BeuiTest<VideoApp>, BlockHandle<Video>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Video::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::new(editor);
    editor.run();
    editor.run();
    (editor, block)
}

fn editor_with_clip() -> (BeuiTest<VideoApp>, BlockHandle<Video>, Uuid) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Video::new());
    let source = client.create_block(Counter::default());
    let clip = Uuid::new_v4();
    block.operate(VideoOperation::InsertClip {
        clip: VideoClip {
            id: clip,
            block_id: BlockRef::Direct(source.id()),
            length: 30,
            attachment: None,
            effects: Vec::new(),
        },
        index: 0,
    });
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::new(editor);
    editor.run();
    editor.run();
    editor.run();
    (editor, block, clip)
}
