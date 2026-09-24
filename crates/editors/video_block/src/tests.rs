use std::sync::Arc;

use block_client::BlockClient;
use block_client::blocks::counter::Counter;
use block_client::blocks::video::Video as VideoBlock;
use block_editor_plugin::be_block::VideoContent;
use block_editor_plugin::be_block::video::{Video, VideoClip, VideoFrameRate, VideoOperation};
use block_editor_plugin::beui::Pos2;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::VideoApp;
use crate::timeline::timecode;

mod an_empty_video_has_nothing_at_the_playhead;
mod clicking_a_clip_selects_it_for_the_effects_panel;
mod dragging_a_clip_past_the_next_one_reorders_the_base_track;
mod timecode_counts_minutes_seconds_and_frames;

struct Fixture {
    client: Arc<BlockClient>,
    editor: ContentHarness<VideoApp>,
}

impl Fixture {
    fn new() -> Self {
        let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
        let block = client.create_block(VideoBlock::new());
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = Editor::new(host.clone(), Arc::clone(&client), block.id());
        let mut editor = ContentHarness::new(BeuiTest::new(editor), host);
        editor.hold(None, VideoContent::default());
        Self { client, editor }
    }

    fn insert(&mut self, index: usize) -> Uuid {
        let source = self.client.create_block(Counter::default());
        let id = Uuid::new_v4();
        let operation = VideoOperation::InsertClip {
            clip: VideoClip {
                id,
                block_id: source.id(),
                length: 30,
                attachment: None,
                effects: Vec::new(),
            },
            index,
        };
        let edit = self
            .editor
            .content::<VideoContent>(None)
            .root()
            .edit_for(&operation);
        self.editor.edit::<VideoContent>(None, &edit);
        id
    }

    fn video(&self) -> Video {
        self.editor.content::<VideoContent>(None).root().video()
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.editor.run();
        }
    }
}

fn editor() -> Fixture {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture
}

fn editor_with_clip() -> (Fixture, Uuid) {
    let mut fixture = Fixture::new();
    let clip = fixture.insert(0);
    fixture.settle();
    (fixture, clip)
}
