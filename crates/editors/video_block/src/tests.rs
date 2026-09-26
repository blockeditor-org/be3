use block_editor_beui::be_block::{BlockContent, CounterContent};

use block_editor_beui::be_block::VideoContent;
use block_editor_beui::be_block::video::{Video, VideoClip, VideoFrameRate, VideoOperation};
use block_editor_beui::beui::Pos2;
use block_editor_beui::{BlockInfo, BlockParent, Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::VideoApp;
use crate::timeline::timecode;

mod an_empty_video_has_nothing_at_the_playhead;
mod clicking_a_clip_selects_it_for_the_effects_panel;
mod dragging_a_clip_past_the_next_one_reorders_the_base_track;
mod timecode_counts_minutes_seconds_and_frames;

struct Fixture {
    editor: BeuiTest<VideoApp>,
}

impl Fixture {
    fn new() -> Self {
        let block = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = Editor::new(host.clone(), block);
        let mut editor = BeuiTest::new(editor);
        editor.hold(None, VideoContent::default());
        Self { editor }
    }

    fn insert(&mut self, index: usize) -> Uuid {
        let source = Uuid::new_v4();
        self.editor.store().add_block(BlockInfo::new(
            source,
            CounterContent::CONTENT_TYPE,
            BlockParent::Detached,
        ));
        let id = Uuid::new_v4();
        let operation = VideoOperation::InsertClip {
            clip: VideoClip {
                id,
                block_id: source,
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
