use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use block_editor_beui::BlockList;
use block_editor_beui::be_block::VideoContent;
use block_editor_beui::be_block::video::{
    DEFAULT_CLIP_SECONDS, Video, VideoAttachment, VideoClip, VideoFrameRate, VideoOperation,
};
use block_editor_beui::beui::reactive::{ReadSignal, WriteSignal, create_signal};
use block_editor_beui::block_ui::{BlockCatalog, BlockLabel};
use block_editor_beui::{BlockFilter, BlockPicker, ChildTarget, ContentProjection, Editor};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};
use uuid::Uuid;

use crate::timeline::{MAX_PIXELS_PER_FRAME, MIN_PIXELS_PER_FRAME};

pub(crate) const DEFAULT_PIXELS_PER_FRAME: f32 = 4.0;
pub(crate) const ZOOM_STEP: f32 = 1.5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClipDrag {
    pub(crate) clip: Uuid,
    pub(crate) grab: u64,
}

type PendingClip = (Uuid, u64, Option<VideoAttachment>, usize);

pub(crate) struct VideoState {
    editor: Editor,
    block: Rc<ContentProjection<VideoContent>>,
    dependencies: BlockList,
    picker: RefCell<BlockPicker>,
    picker_attachment: Cell<Option<Uuid>>,
    pending_clips: RefCell<Vec<(Uuid, PendingClip)>>,
    play_origin: Cell<Option<(Instant, u64)>>,
    aspect_ratios: RefCell<HashMap<Uuid, f32>>,
    fit_requested: Cell<bool>,
    pub(crate) clips: ReadSignal<Vec<VideoClip>>,
    pub(crate) duration: ReadSignal<u64>,
    pub(crate) frame_rate: ReadSignal<VideoFrameRate>,
    pub(crate) selected: ReadSignal<Option<Uuid>>,
    set_selected: WriteSignal<Option<Uuid>>,
    pub(crate) playhead: ReadSignal<u64>,
    set_playhead: WriteSignal<u64>,
    pub(crate) playing: ReadSignal<bool>,
    set_playing: WriteSignal<bool>,
    pub(crate) pixels_per_frame: ReadSignal<f32>,
    set_pixels_per_frame: WriteSignal<f32>,
    pub(crate) drag: ReadSignal<Option<ClipDrag>>,
    set_drag: WriteSignal<Option<ClipDrag>>,
    pub(crate) labels: ReadSignal<HashMap<Uuid, BlockLabel>>,
    set_labels: WriteSignal<HashMap<Uuid, BlockLabel>>,
}

impl VideoState {
    pub(crate) fn new(editor: &Editor) -> Rc<Self> {
        let block = editor.block_content::<VideoContent>();
        let clips = block.project(|video| video.root().video().clips().to_vec());
        let duration = block.project(|video| video.root().video().duration());
        let frame_rate = block.project(|video| video.root().frame_rate);
        let (selected, set_selected) = create_signal(None);
        let (playhead, set_playhead) = create_signal(0);
        let (playing, set_playing) = create_signal(false);
        let (pixels_per_frame, set_pixels_per_frame) = create_signal(DEFAULT_PIXELS_PER_FRAME);
        let (drag, set_drag) = create_signal(None);
        let (labels, set_labels) = create_signal(HashMap::new());
        Rc::new(Self {
            dependencies: editor
                .blocks()
                .watch(BlockQuery::References(editor.block_id())),
            editor: editor.clone(),
            block,
            picker: RefCell::new(BlockPicker::default()),
            picker_attachment: Cell::new(None),
            pending_clips: RefCell::new(Vec::new()),
            play_origin: Cell::new(None),
            aspect_ratios: RefCell::new(HashMap::new()),
            fit_requested: Cell::new(false),
            clips,
            duration,
            frame_rate,
            selected,
            set_selected,
            playhead,
            set_playhead,
            playing,
            set_playing,
            pixels_per_frame,
            set_pixels_per_frame,
            drag,
            set_drag,
            labels,
            set_labels,
        })
    }

    pub(crate) fn editor(&self) -> &Editor {
        &self.editor
    }

    pub(crate) fn block_id(&self) -> Uuid {
        self.editor.block_id()
    }

    pub(crate) fn video(&self) -> Option<Video> {
        self.block.read(|video| video.root().video())
    }

    pub(crate) fn operate(&self, operation: VideoOperation) {
        if let Some(edit) = self.block.read(|video| video.root().edit_for(&operation)) {
            self.block.operate(edit);
        }
    }

    pub(crate) fn update_clip(&self, clip: VideoClip) {
        self.operate(VideoOperation::UpdateClips { clips: vec![clip] });
    }

    pub(crate) fn select(&self, clip: Option<Uuid>) {
        self.set_selected.set(clip);
    }

    pub(crate) fn selected_clip(&self) -> Option<VideoClip> {
        let id = self.selected.get()?;
        self.clips.get().into_iter().find(|clip| clip.id == id)
    }

    pub(crate) fn begin_drag(&self, drag: Option<ClipDrag>) {
        self.set_drag.set(drag);
    }

    pub(crate) fn target(&self, block: Uuid) -> Option<ChildTarget> {
        let label = self.labels.get().get(&block).cloned()?;
        Some(ChildTarget::new(block, label.block_type))
    }

    pub(crate) fn name_of(&self, block: Uuid) -> String {
        match self.labels.get().get(&block) {
            Some(label) => label.name.clone(),
            None => "Loading…".to_owned(),
        }
    }

    pub(crate) fn report_aspect_ratio(&self, id: Uuid, ratio: f32) {
        self.aspect_ratios.borrow_mut().insert(id, ratio);
    }

    pub(crate) fn aspect_ratio(&self, id: Uuid) -> Option<f32> {
        self.aspect_ratios.borrow().get(&id).copied()
    }

    pub(crate) fn seek(&self, frame: u64) {
        self.set_playhead
            .set(frame.min(self.duration.get_untracked()));
        self.play_origin.set(None);
    }

    pub(crate) fn toggle_playback(&self) {
        let duration = self.duration.get_untracked();
        let playing = !self.playing.get_untracked();
        self.set_playing.set(playing);
        self.play_origin.set(None);
        if playing && self.playhead.get_untracked() >= duration {
            self.set_playhead.set(0);
        }
    }

    pub(crate) fn zoom_timeline(&self, factor: f32) {
        let next = (self.pixels_per_frame.get_untracked() * factor)
            .clamp(MIN_PIXELS_PER_FRAME, MAX_PIXELS_PER_FRAME);
        self.set_pixels_per_frame.set(next);
    }

    pub(crate) fn request_fit(&self) {
        self.fit_requested.set(true);
    }

    pub(crate) fn fit_timeline(&self, width: f32) {
        if !self.fit_requested.take() {
            return;
        }
        let duration = self.duration.get_untracked();
        if duration == 0 {
            return;
        }
        let next = ((width - crate::timeline::TAIL_PADDING * 0.25).max(160.0) / duration as f32)
            .clamp(MIN_PIXELS_PER_FRAME, MAX_PIXELS_PER_FRAME);
        self.set_pixels_per_frame.set(next);
    }

    pub(crate) fn open_picker(&self, attachment: Option<Uuid>) {
        self.picker_attachment.set(attachment);
        self.picker
            .borrow_mut()
            .open(self.editor.host(), BlockFilter::default());
    }

    pub(crate) fn split_selected(&self) {
        let (Some(video), Some(selected)) = (self.video(), self.selected.get_untracked()) else {
            return;
        };
        let (Some(clip), Some(timing)) = (video.clip(selected), video.timing(selected)) else {
            return;
        };
        let playhead = self.playhead.get_untracked();
        if !timing.covers(playhead) || playhead == timing.start {
            return;
        }
        let first_length = playhead - timing.start;
        let mut first = clip.clone();
        first.length = first_length;
        let mut second = clip.clone();
        second.id = Uuid::new_v4();
        second.length = timing.length - first_length;
        second.attachment = clip.attachment.map(|attachment| {
            VideoAttachment::new(
                attachment.clip_id,
                attachment.offset + i64::try_from(first_length).unwrap_or(i64::MAX),
            )
        });
        let index = video.sibling_index(selected).unwrap_or(0) + 1;
        self.set_selected.set(Some(second.id));
        self.operate(VideoOperation::UpdateClips { clips: vec![first] });
        self.operate(VideoOperation::InsertClip {
            clip: second,
            index,
        });
    }

    pub(crate) fn remove_selected(&self) {
        let Some(id) = self.selected.get_untracked() else {
            return;
        };
        self.operate(VideoOperation::RemoveClips { ids: vec![id] });
        self.set_selected.set(None);
    }

    pub(crate) fn insert_clip(
        &self,
        block_id: Uuid,
        attachment: Option<Uuid>,
        frame: u64,
        index: Option<usize>,
    ) {
        let Some(video) = self.video() else {
            return;
        };
        let length = video.frame_rate().frames(DEFAULT_CLIP_SECONDS).max(1);
        let (attachment, index) = match attachment {
            Some(parent) => {
                let start = video.timing(parent).map_or(0, |timing| timing.start);
                let offset =
                    i64::try_from(frame).unwrap_or(i64::MAX) - i64::try_from(start).unwrap_or(0);
                (
                    Some(VideoAttachment::new(parent, offset)),
                    index.unwrap_or_else(|| video.children(Some(parent)).len()),
                )
            }
            None => (None, index.unwrap_or_else(|| video.children(None).len())),
        };
        let clip_id = Uuid::new_v4();
        self.set_selected.set(Some(clip_id));
        self.pending_clips
            .borrow_mut()
            .push((block_id, (clip_id, length, attachment, index)));
    }

    pub(crate) fn adopt(&self, block_id: Uuid) {
        self.editor
            .blocks()
            .set_parent(block_id, BlockParent::Block(self.block_id()));
    }

    pub(crate) fn poll(&self) {
        self.poll_pending_clips();
        self.poll_picker();
        self.publish_references();
        self.synchronize();
        self.advance_playback();
    }

    fn synchronize(&self) {
        let duration = self.duration.get_untracked();
        let clips = self.clips.get_untracked();
        if let Some(selected) = self.selected.get_untracked()
            && !clips.iter().any(|clip| clip.id == selected)
        {
            self.set_selected.set(None);
        }
        if self.playhead.get_untracked() > duration {
            self.set_playhead.set(duration);
        }
    }

    fn advance_playback(&self) {
        if !self.playing.get_untracked() {
            return;
        }
        let duration = self.duration.get_untracked();
        if duration == 0 {
            self.set_playing.set(false);
            return;
        }
        let now = Instant::now();
        let (origin_time, origin_frame) = self
            .play_origin
            .get()
            .unwrap_or((now, self.playhead.get_untracked()));
        self.play_origin.set(Some((origin_time, origin_frame)));
        let elapsed = now.duration_since(origin_time).as_secs_f64();
        let frame = origin_frame.saturating_add(self.frame_rate.get_untracked().frames(elapsed));
        if frame >= duration {
            self.set_playhead.set(duration);
            self.set_playing.set(false);
            self.play_origin.set(None);
        } else {
            self.set_playhead.set(frame);
        }
        self.editor.host().waker().wake();
    }

    fn poll_pending_clips(&self) {
        let landed = std::mem::take(&mut *self.pending_clips.borrow_mut());
        for (reference, (clip_id, length, attachment, index)) in landed {
            self.operate(VideoOperation::InsertClip {
                clip: VideoClip {
                    id: clip_id,
                    block_id: reference,
                    length,
                    attachment,
                    effects: Vec::new(),
                },
                index,
            });
        }
    }

    fn poll_picker(&self) {
        let picked = self.picker.borrow_mut().poll(self.editor.host());
        let Some(Ok(picked)) = picked else {
            return;
        };
        let attachment = self.picker_attachment.take();
        self.adopt(picked.id);
        self.insert_clip(picked.id, attachment, self.playhead.get_untracked(), None);
    }

    fn publish_references(&self) {
        let types: Rc<BlockCatalog> = self.editor.host().block_types();
        let labels: HashMap<Uuid, BlockLabel> = self
            .dependencies
            .read()
            .into_iter()
            .map(|reference: BlockInfo| (reference.id, reference.label(types.as_ref())))
            .collect();
        if self.labels.get_untracked() != labels {
            self.set_labels.set(labels);
        }
    }
}
