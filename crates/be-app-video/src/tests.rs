use be_sdk_beui::be_block::BlockContent;
use be_type_example_counter::CounterContent;

use be_sdk_beui::beui::{Pos2, Vec2};
use be_sdk_beui::{BlockInfo, BlockParent};
use be_sdk_test::BeuiTest;
use be_type_video::VideoContent;
use be_type_video::{Video, VideoClip, VideoFrameRate, VideoOperation};
use uuid::Uuid;

use crate::root::VideoRoot;
use crate::timeline::timecode;

mod a_trim_keeps_following_the_pointer_once_its_first_edit_lands;
mod a_video_on_a_phone_on_its_side_shows_its_panes_beside_the_player;
mod a_video_on_a_phone_shows_its_panes_below_the_player;
mod an_empty_video_has_nothing_at_the_playhead;
mod clicking_a_clip_selects_it_for_the_effects_panel;
mod dragging_a_clip_past_the_next_one_reorders_the_base_track;
mod dragging_a_trim_handle_lengthens_its_clip_by_how_far_it_moved;
mod timecode_counts_minutes_seconds_and_frames;

struct Fixture {
    editor: BeuiTest<VideoRoot>,
}

impl Fixture {
    fn new() -> Self {
        Self::start(BeuiTest::editing(Uuid::new_v4()))
    }

    fn sized(size: Vec2) -> Self {
        Self::start(BeuiTest::editing(Uuid::new_v4()).with_size(size))
    }

    fn start(mut editor: BeuiTest<VideoRoot>) -> Self {
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
        self.editor.run();
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
