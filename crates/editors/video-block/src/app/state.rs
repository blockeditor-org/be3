use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use block_editor_beui::beui::reactive::*;
use block_editor_beui::block_ui::{BlockCatalog, BlockLabel};
use block_editor_beui::{BlockFilter, ChildTarget, ContentProjection, Editor};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};
use uuid::Uuid;
use video_block_types::VideoContent;
use video_block_types::{
    DEFAULT_CLIP_SECONDS, Video, VideoAttachment, VideoClip, VideoFrameRate, VideoOperation,
};

use crate::timeline::{MAX_PIXELS_PER_FRAME, MIN_PIXELS_PER_FRAME};

pub(crate) const DEFAULT_PIXELS_PER_FRAME: f32 = 4.0;
pub(crate) const ZOOM_STEP: f32 = 1.5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClipDrag {
    pub(crate) clip: Uuid,
    pub(crate) grab: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct VideoState {
    editor: Editor,
    fit_requested: Signal<bool>,
    pub(crate) clips: ReadSignal<Vec<VideoClip>>,
    pub(crate) duration: ReadSignal<u64>,
    pub(crate) frame_rate: ReadSignal<VideoFrameRate>,
    pub(crate) selected: Signal<Option<Uuid>>,
    pub(crate) playhead: Signal<u64>,
    pub(crate) playing: Signal<bool>,
    pub(crate) pixels_per_frame: Signal<f32>,
    pub(crate) drag: Signal<Option<ClipDrag>>,
    pub(crate) labels: Memo<Ready<HashMap<Uuid, BlockLabel>>>,
    inner: Stored<Inner>,
}

struct Inner {
    block: ContentProjection<VideoContent>,
    play_origin: Cell<Option<(Instant, u64)>>,
    aspect_ratios: RefCell<HashMap<Uuid, f32>>,
}

impl VideoState {
    pub(crate) fn new(editor: Editor) -> Self {
        let block = editor.block_content::<VideoContent>();
        let clips = block.project(|video| video.root().video().clips().to_vec());
        let duration = block.project(|video| video.root().video().duration());
        let frame_rate = block.project(|video| video.root().frame_rate);
        let selected = create_signal(None);
        let playhead = create_signal(0);
        let playing = create_signal(false);
        let pixels_per_frame = create_signal(DEFAULT_PIXELS_PER_FRAME);
        let drag = create_signal(None);
        let dependencies = editor
            .blocks()
            .watch(BlockQuery::References(editor.block_id()));
        let labels = create_memo(move || {
            let types: Rc<BlockCatalog> = editor.block_types();
            Ok(dependencies
                .read()?
                .into_iter()
                .map(|reference: BlockInfo| (reference.id, reference.label(types.as_ref())))
                .collect())
        });
        let fit_requested = create_signal(false);
        let state = Self {
            editor,
            fit_requested,
            clips,
            duration,
            frame_rate,
            selected,
            playhead,
            playing,
            pixels_per_frame,
            drag,
            labels,
            inner: store(Inner {
                block,
                play_origin: Cell::new(None),
                aspect_ratios: RefCell::new(HashMap::new()),
            }),
        };
        create_effect(move || state.synchronize());
        let playback = create_timer(move || state.advance_playback());
        let playing = state.playing;
        create_effect(move || {
            if playing.get() {
                playback.start(Duration::ZERO);
            }
        });
        state
    }

    pub(crate) fn editor(&self) -> Editor {
        self.editor
    }

    pub(crate) fn block_id(&self) -> Uuid {
        self.editor.block_id()
    }

    pub(crate) fn video(&self) -> Option<Video> {
        let inner = self.inner.value();
        inner.block.read(|video| video.root().video()).ok()
    }

    pub(crate) fn operate(&self, operation: VideoOperation) {
        let inner = self.inner.value();
        if let Ok(edit) = inner.block.read(|video| video.root().edit_for(&operation)) {
            inner.block.operate(edit);
        }
    }

    pub(crate) fn update_clip(&self, clip: VideoClip) {
        self.operate(VideoOperation::UpdateClips { clips: vec![clip] });
    }

    pub(crate) fn select(&self, clip: Option<Uuid>) {
        self.selected.set(clip);
    }

    pub(crate) fn selected_clip(&self) -> Option<VideoClip> {
        let id = self.selected.get()?;
        self.clips.get().into_iter().find(|clip| clip.id == id)
    }

    pub(crate) fn begin_drag(&self, drag: Option<ClipDrag>) {
        self.drag.set(drag);
    }

    pub(crate) fn target(&self, block: Uuid) -> Option<ChildTarget> {
        let label = self.labels.get().ok()?.get(&block).cloned()?;
        Some(ChildTarget::new(block, label.block_type))
    }

    pub(crate) fn name_of(&self, block: Uuid) -> String {
        match self.labels.get() {
            Ok(labels) => labels
                .get(&block)
                .map_or_else(|| "Loading…".to_owned(), |label| label.name.clone()),
            Err(Pending::Loading) => "Loading…".to_owned(),
            Err(Pending::Failed(error)) => error,
        }
    }

    pub(crate) fn report_aspect_ratio(&self, id: Uuid, ratio: f32) {
        let inner = self.inner.value();
        inner.aspect_ratios.borrow_mut().insert(id, ratio);
    }

    pub(crate) fn aspect_ratio(&self, id: Uuid) -> Option<f32> {
        let inner = self.inner.value();
        inner.aspect_ratios.borrow().get(&id).copied()
    }

    pub(crate) fn seek(&self, frame: u64) {
        let inner = self.inner.value();
        self.playhead.set(frame.min(self.duration.get_untracked()));
        inner.play_origin.set(None);
    }

    pub(crate) fn toggle_playback(&self) {
        let inner = self.inner.value();
        let duration = self.duration.get_untracked();
        let playing = !self.playing.get_untracked();
        self.playing.set(playing);
        inner.play_origin.set(None);
        if playing && self.playhead.get_untracked() >= duration {
            self.playhead.set(0);
        }
    }

    pub(crate) fn zoom_timeline(&self, factor: f32) {
        let next = (self.pixels_per_frame.get_untracked() * factor)
            .clamp(MIN_PIXELS_PER_FRAME, MAX_PIXELS_PER_FRAME);
        self.pixels_per_frame.set(next);
    }

    pub(crate) fn request_fit(&self) {
        self.fit_requested.set(true);
    }

    pub(crate) fn fit_timeline(&self, width: f32) {
        if !self.fit_requested.get() {
            return;
        }
        self.fit_requested.set(false);
        let duration = self.duration.get_untracked();
        if duration == 0 {
            return;
        }
        let next = ((width - crate::timeline::TAIL_PADDING * 0.25).max(160.0) / duration as f32)
            .clamp(MIN_PIXELS_PER_FRAME, MAX_PIXELS_PER_FRAME);
        self.pixels_per_frame.set(next);
    }

    pub(crate) fn open_picker(&self, attachment: Option<Uuid>) {
        let state = *self;
        self.editor
            .pick_block(BlockFilter::default(), move |picked| {
                let Ok(picked) = picked else {
                    return;
                };
                if state.inner.is_alive() {
                    state.adopt(picked.id());
                    state.insert_clip(picked.id(), attachment, state.playhead.get_untracked(), None);
                }
            });
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
        self.selected.set(Some(second.id));
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
        self.selected.set(None);
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
        self.operate(VideoOperation::InsertClip {
            clip: VideoClip {
                id: clip_id,
                block_id,
                length,
                attachment,
                effects: Vec::new(),
            },
            index,
        });
        self.selected.set(Some(clip_id));
    }

    pub(crate) fn adopt(&self, block_id: Uuid) {
        self.editor
            .blocks()
            .set_parent(block_id, BlockParent::Block(self.block_id()));
    }

    fn synchronize(&self) {
        let duration = self.duration.get();
        let clips = self.clips.get();
        if let Some(selected) = self.selected.get()
            && !clips.iter().any(|clip| clip.id == selected)
        {
            self.selected.set(None);
        }
        if self.playhead.get() > duration {
            self.playhead.set(duration);
        }
    }

    fn advance_playback(&self) -> Option<Duration> {
        let inner = self.inner.value();
        if !self.playing.get_untracked() {
            return None;
        }
        let duration = self.duration.get_untracked();
        if duration == 0 {
            self.playing.set(false);
            return None;
        }
        let now = Instant::now();
        let (origin_time, origin_frame) = inner
            .play_origin
            .get()
            .unwrap_or((now, self.playhead.get_untracked()));
        inner.play_origin.set(Some((origin_time, origin_frame)));
        let elapsed = now.duration_since(origin_time).as_secs_f64();
        let frame = origin_frame.saturating_add(self.frame_rate.get_untracked().frames(elapsed));
        if frame >= duration {
            self.playhead.set(duration);
            self.playing.set(false);
            inner.play_origin.set(None);
            return None;
        }
        self.playhead.set(frame);
        Some(Duration::ZERO)
    }
}
