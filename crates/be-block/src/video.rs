use std::collections::HashSet;

use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ChildChange, Root};

pub const DEFAULT_CLIP_SECONDS: f64 = 5.0;

pub const MAX_CLIP_LENGTH: u64 = 1_000_000;

const MAX_FRAME_RATE_PART: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VideoFrameRate {
    pub numerator: u32,
    pub denominator: u32,
}

impl VideoFrameRate {
    pub const DEFAULT: Self = Self::new(60, 1);

    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub fn frames_per_second(self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator)
    }

    pub fn seconds(self, frames: u64) -> f64 {
        frames as f64 / self.frames_per_second()
    }

    pub fn frames(self, seconds: f64) -> u64 {
        if !seconds.is_finite() || seconds <= 0.0 {
            return 0;
        }
        (seconds * self.frames_per_second()) as u64
    }

    fn normalized(self) -> Self {
        Self::new(
            self.numerator.clamp(1, MAX_FRAME_RATE_PART),
            self.denominator.clamp(1, MAX_FRAME_RATE_PART),
        )
    }
}

impl Default for VideoFrameRate {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VideoAttachment {
    pub clip_id: Uuid,
    pub offset: i64,
}

impl VideoAttachment {
    pub const fn new(clip_id: Uuid, offset: i64) -> Self {
        Self { clip_id, offset }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VideoEffect {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VideoClip {
    pub id: Uuid,

    pub block_id: Uuid,

    pub length: u64,

    pub attachment: Option<VideoAttachment>,
    pub effects: Vec<VideoEffect>,
}

impl VideoClip {
    pub fn new(block_id: Uuid, length: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
            block_id,
            length,
            attachment: None,
            effects: Vec::new(),
        }
    }

    pub fn attached_to(mut self, clip_id: Uuid, offset: i64) -> Self {
        self.attachment = Some(VideoAttachment::new(clip_id, offset));
        self
    }

    pub fn parent(&self) -> Option<Uuid> {
        self.attachment.map(|attachment| attachment.clip_id)
    }

    fn offset(&self) -> i64 {
        self.attachment.map_or(0, |attachment| attachment.offset)
    }

    fn normalized(mut self) -> Self {
        self.length = self.length.clamp(1, MAX_CLIP_LENGTH);
        self
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VideoClipTiming {
    pub id: Uuid,
    pub start: u64,
    pub length: u64,

    pub depth: usize,
}

impl VideoClipTiming {
    pub fn end(&self) -> u64 {
        self.start.saturating_add(self.length)
    }

    pub fn covers(&self, frame: u64) -> bool {
        frame >= self.start && frame < self.end()
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Video {
    frame_rate: VideoFrameRate,
    clips: Vec<VideoClip>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum VideoOperation {
    InsertClip { clip: VideoClip, index: usize },

    RemoveClips { ids: Vec<Uuid> },

    UpdateClips { clips: Vec<VideoClip> },

    MoveClip { clip_id: Uuid, index: usize },
    SetFrameRate { frame_rate: VideoFrameRate },
}

impl Video {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn frame_rate(&self) -> VideoFrameRate {
        self.frame_rate
    }

    pub fn clips(&self) -> &[VideoClip] {
        &self.clips
    }

    pub fn clip(&self, id: Uuid) -> Option<&VideoClip> {
        self.clips.iter().find(|clip| clip.id == id)
    }

    pub fn children(&self, parent: Option<Uuid>) -> Vec<&VideoClip> {
        self.clips
            .iter()
            .filter(|clip| clip.parent() == parent)
            .collect()
    }

    pub fn sibling_index(&self, clip_id: Uuid) -> Option<usize> {
        let clip = self.clip(clip_id)?;
        self.children(clip.parent())
            .iter()
            .position(|sibling| sibling.id == clip_id)
    }

    pub fn timeline(&self) -> Vec<VideoClipTiming> {
        let mut timings = Vec::with_capacity(self.clips.len());
        let mut visited = HashSet::new();
        let mut start = 0;
        for clip in self.children(None) {
            self.push_timings(clip, start, 0, &mut visited, &mut timings);
            start = start.saturating_add(clip.length);
        }
        timings
    }

    fn push_timings(
        &self,
        clip: &VideoClip,
        start: u64,
        depth: usize,
        visited: &mut HashSet<Uuid>,
        timings: &mut Vec<VideoClipTiming>,
    ) {
        if !visited.insert(clip.id) {
            return;
        }
        timings.push(VideoClipTiming {
            id: clip.id,
            start,
            length: clip.length,
            depth,
        });
        for child in self.children(Some(clip.id)) {
            let child_start = start.saturating_add_signed(child.offset());
            self.push_timings(child, child_start, depth + 1, visited, timings);
        }
    }

    pub fn duration(&self) -> u64 {
        self.timeline()
            .iter()
            .map(VideoClipTiming::end)
            .max()
            .unwrap_or(0)
    }

    pub fn timing(&self, clip_id: Uuid) -> Option<VideoClipTiming> {
        self.timeline()
            .into_iter()
            .find(|timing| timing.id == clip_id)
    }

    pub fn visible_at(&self, frame: u64) -> Vec<Uuid> {
        self.timeline()
            .iter()
            .filter(|timing| timing.covers(frame))
            .map(|timing| timing.id)
            .collect()
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct VideoProject {
    pub frame_rate: VideoFrameRate,
    pub clips: List<ClipNode>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct ClipNode {
    pub block: Option<Uuid>,
    pub length: u64,
    pub offset: i64,
    pub effects: Vec<VideoEffect>,
    pub attached: List<ClipNode>,
}

impl VideoProject {
    pub fn video(&self) -> Video {
        let mut clips = Vec::new();
        flatten(&self.clips, None, &mut clips);
        Video {
            frame_rate: self.frame_rate,
            clips,
        }
    }

    pub fn edit_for(&self, operation: &VideoOperation) -> Edit {
        let video = self.video();
        match operation {
            VideoOperation::InsertClip { clip, index } => {
                if video.clip(clip.id).is_some() {
                    return Edit::default();
                }
                let clip = clip.clone().normalized();
                let parent = clip.parent().filter(|parent| video.clip(*parent).is_some());
                let anchor = sibling_anchor(&video, parent, *index, None);
                let node = ClipNode {
                    block: Some(clip.block_id),
                    length: clip.length,
                    offset: parent.map_or(0, |_| clip.offset()),
                    effects: clip.effects,
                    attached: List::default(),
                };
                let id = ObjectId::from_uuid(clip.id);
                match parent {
                    Some(parent) => {
                        ClipNode::ATTACHED.insert_as(id, ObjectId::from_uuid(parent), anchor, &node)
                    }
                    None => Self::CLIPS.insert_as(id, ObjectId::ROOT, anchor, &node),
                }
                .into()
            }
            VideoOperation::RemoveClips { ids } => ids
                .iter()
                .map(|id| Change::remove(ObjectId::from_uuid(*id)))
                .collect(),
            VideoOperation::UpdateClips { clips } => clips
                .iter()
                .filter_map(|update| {
                    let existing = video.clip(update.id)?;
                    let update = update.clone().normalized();
                    let id = ObjectId::from_uuid(update.id);
                    let mut changes = vec![
                        ClipNode::BLOCK.set(id, &Some(update.block_id)),
                        ClipNode::LENGTH.set(id, &update.length),
                        ClipNode::EFFECTS.set(id, &update.effects),
                    ];
                    let accepted = match update.parent() {
                        None => true,
                        Some(parent) => {
                            video.clip(parent).is_some() && !video.attaches_to(parent, update.id)
                        }
                    };
                    if !accepted {
                        return Some(changes);
                    }
                    if update.parent().is_some() {
                        changes.push(ClipNode::OFFSET.set(id, &update.offset()));
                    }
                    if update.parent() != existing.parent() {
                        changes.push(match update.parent() {
                            Some(parent) => ClipNode::ATTACHED.move_into(
                                ObjectId::from_uuid(parent),
                                Anchor::End,
                                id,
                            ),
                            None => Self::CLIPS.move_into(ObjectId::ROOT, Anchor::End, id),
                        });
                    }
                    Some(changes)
                })
                .flatten()
                .collect(),
            VideoOperation::MoveClip { clip_id, index } => {
                let Some(clip) = video.clip(*clip_id) else {
                    return Edit::default();
                };
                let parent = clip.parent();
                let anchor = sibling_anchor(&video, parent, *index, Some(*clip_id));
                let id = ObjectId::from_uuid(*clip_id);
                match parent {
                    Some(parent) => {
                        ClipNode::ATTACHED.move_into(ObjectId::from_uuid(parent), anchor, id)
                    }
                    None => Self::CLIPS.move_into(ObjectId::ROOT, anchor, id),
                }
                .into()
            }
            VideoOperation::SetFrameRate { frame_rate } => Self::FRAME_RATE
                .set(ObjectId::ROOT, &frame_rate.normalized())
                .into(),
        }
    }
}

impl Video {
    fn attaches_to(&self, clip: Uuid, ancestor: Uuid) -> bool {
        let mut visited = HashSet::new();
        let mut current = Some(clip);
        while let Some(id) = current {
            if id == ancestor || !visited.insert(id) {
                return true;
            }
            current = self.clip(id).and_then(VideoClip::parent);
        }
        false
    }
}

fn flatten(nodes: &[be_model::Item<ClipNode>], parent: Option<Uuid>, clips: &mut Vec<VideoClip>) {
    for node in nodes {
        let id = node.id.as_uuid();
        if let Some(block_id) = node.block {
            clips.push(VideoClip {
                id,
                block_id,
                length: node.length,
                attachment: parent.map(|parent| VideoAttachment::new(parent, node.offset)),
                effects: node.effects.clone(),
            });
        }
        flatten(&node.attached, Some(id), clips);
    }
}

fn sibling_anchor(
    video: &Video,
    parent: Option<Uuid>,
    index: usize,
    skipping: Option<Uuid>,
) -> Anchor {
    let siblings: Vec<Uuid> = video
        .children(parent)
        .into_iter()
        .map(|clip| clip.id)
        .filter(|id| Some(*id) != skipping)
        .take(index)
        .collect();
    siblings
        .last()
        .map_or(Anchor::Start, |id| Anchor::After(ObjectId::from_uuid(*id)))
}

impl Root for VideoProject {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x7669_6465_6f2d_636f_6e74_656e_7400_0002);

    fn references(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.video()
            .clips
            .iter()
            .map(|clip| clip.block_id)
            .filter(|block| seen.insert(*block))
            .collect()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        let video = self.video();
        let showing = |block: Uuid| {
            video
                .clips()
                .iter()
                .filter(move |clip| clip.block_id == block)
                .cloned()
        };
        Some(
            self.edit_for(&match change {
                ChildChange::Add(block) => VideoOperation::InsertClip {
                    clip: VideoClip::new(
                        block,
                        video.frame_rate().frames(DEFAULT_CLIP_SECONDS).max(1),
                    ),
                    index: video.children(None).len(),
                },
                ChildChange::Delete(block) => VideoOperation::RemoveClips {
                    ids: showing(block).map(|clip| clip.id).collect(),
                },
                ChildChange::Replace { old, new } => VideoOperation::UpdateClips {
                    clips: showing(old)
                        .map(|clip| VideoClip {
                            block_id: new,
                            ..clip
                        })
                        .collect(),
                },
            }),
        )
    }
}

pub type VideoContent = Document<VideoProject>;
