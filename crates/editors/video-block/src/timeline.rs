use block_editor_beui::be_block::video::{
    Video, VideoAttachment, VideoClip, VideoClipTiming, VideoFrameRate, VideoOperation,
};
use block_editor_beui::beui::{Pos2, Rect};
use uuid::Uuid;

use block_editor_beui::be_block::video::DEFAULT_CLIP_SECONDS;

use crate::app::state::ClipDrag;

pub(crate) const MIN_PIXELS_PER_FRAME: f32 = 0.02;
pub(crate) const MAX_PIXELS_PER_FRAME: f32 = 40.0;

pub(crate) const RULER_HEIGHT: f32 = 22.0;
pub(crate) const LANE_HEIGHT: f32 = 38.0;
pub(crate) const LANE_GAP: f32 = 3.0;
pub(crate) const TRIM_HANDLE_WIDTH: f32 = 6.0;

pub(crate) const TAIL_PADDING: f32 = 240.0;
const MIN_TICK_SPACING: f64 = 64.0;
const TICK_SECONDS: [f64; 12] = [
    1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0,
];

pub(crate) struct ClipRow {
    pub(crate) timing: VideoClipTiming,
    pub(crate) lane: usize,
}

pub(crate) fn timecode(frame_rate: VideoFrameRate, frame: u64) -> String {
    let fps = (frame_rate.frames_per_second().round() as u64).max(1);
    let seconds = frame / fps;
    format!("{}:{:02}.{:02}", seconds / 60, seconds % 60, frame % fps)
}

pub(crate) fn lane_rows(video: &Video) -> Vec<ClipRow> {
    let mut next_lane = 1;
    video
        .timeline()
        .into_iter()
        .map(|timing| {
            let lane = if timing.depth == 0 {
                0
            } else {
                next_lane += 1;
                next_lane - 1
            };
            ClipRow { timing, lane }
        })
        .collect()
}

pub(crate) fn lane_rect(content: Rect, lane: usize) -> Rect {
    let top = content.top() + RULER_HEIGHT + lane as f32 * (LANE_HEIGHT + LANE_GAP);
    Rect::from_min_max(
        Pos2::new(content.left(), top),
        Pos2::new(content.right(), top + LANE_HEIGHT),
    )
}

fn lane_at(content: Rect, y: f32) -> usize {
    ((y - content.top() - RULER_HEIGHT) / (LANE_HEIGHT + LANE_GAP))
        .floor()
        .max(0.0) as usize
}

#[derive(Clone, Copy)]
enum ClipDropZone {
    Before,
    Center(Rect),
    After,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum TimelineDropTarget {
    Attach {
        parent: Uuid,
        start: u64,
        lane: usize,
        length: u64,
        highlight: Rect,
    },
    Base {
        index: usize,
        x: f32,
    },
    Offset {
        start: u64,
        lane: usize,
        length: u64,
    },
}

pub(crate) fn clip_rect(content: Rect, row: &ClipRow, pixels_per_frame: f32) -> Rect {
    let lane = lane_rect(content, row.lane);
    let left = content.left() + row.timing.start as f32 * pixels_per_frame;
    Rect::from_min_max(
        Pos2::new(left, lane.top()),
        Pos2::new(
            left + (row.timing.length as f32 * pixels_per_frame).max(3.0),
            lane.bottom(),
        ),
    )
}

fn drop_zone(rect: Rect, x: f32) -> ClipDropZone {
    const EDGE_WIDTH: f32 = 10.0;
    if rect.width() <= EDGE_WIDTH * 2.0 {
        return if x < rect.center().x {
            ClipDropZone::Before
        } else {
            ClipDropZone::After
        };
    }
    if x < rect.left() + EDGE_WIDTH {
        ClipDropZone::Before
    } else if x > rect.right() - EDGE_WIDTH {
        ClipDropZone::After
    } else {
        ClipDropZone::Center(Rect::from_min_max(
            Pos2::new(rect.left() + EDGE_WIDTH, rect.top()),
            Pos2::new(rect.right() - EDGE_WIDTH, rect.bottom()),
        ))
    }
}

fn would_create_cycle(video: &Video, clip_id: Uuid, parent: Uuid) -> bool {
    let mut current = Some(parent);
    while let Some(id) = current {
        if id == clip_id {
            return true;
        }
        current = video.clip(id).and_then(VideoClip::parent);
    }
    false
}

fn adjusted_base_index(video: &Video, boundary: usize, moving: Option<Uuid>) -> usize {
    let moving_index = moving.and_then(|clip_id| {
        video
            .clip(clip_id)
            .filter(|clip| clip.attachment.is_none())
            .and_then(|_| video.sibling_index(clip_id))
    });
    boundary.saturating_sub(usize::from(
        moving_index.is_some_and(|moving_index| moving_index < boundary),
    ))
}

fn base_target(
    video: &Video,
    rows: &[ClipRow],
    content: Rect,
    pixels_per_frame: f32,
    pointer_x: f32,
    moving: Option<Uuid>,
) -> TimelineDropTarget {
    let mut boundaries = Vec::new();
    for row in rows.iter().filter(|row| row.timing.depth == 0) {
        if moving == Some(row.timing.id) {
            continue;
        }
        let rect = clip_rect(content, row, pixels_per_frame);
        let index = video.sibling_index(row.timing.id).unwrap_or(0);
        boundaries.push((rect.left(), index));
        boundaries.push((rect.right(), index + 1));
    }
    let (x, boundary) = boundaries
        .into_iter()
        .min_by(|left, right| {
            (left.0 - pointer_x)
                .abs()
                .total_cmp(&(right.0 - pointer_x).abs())
        })
        .unwrap_or((content.left(), 0));
    TimelineDropTarget::Base {
        index: adjusted_base_index(video, boundary, moving),
        x,
    }
}

pub(crate) fn drop_target_at(
    video: &Video,
    rows: &[ClipRow],
    content: Rect,
    pixels_per_frame: f32,
    pointer: Pos2,
    moving: Option<&ClipDrag>,
) -> Option<TimelineDropTarget> {
    let moving_id = moving.map(|drag| drag.clip);
    let pointer_frame = ((pointer.x - content.left()) / pixels_per_frame).max(0.0) as u64;
    let moved_start = pointer_frame.saturating_sub(moving.map_or(0, |drag| drag.grab));
    let dragged_length = moving.and_then(|drag| video.clip(drag.clip)).map_or_else(
        || video.frame_rate().frames(DEFAULT_CLIP_SECONDS).max(1),
        |clip| clip.length,
    );

    if let Some((row, rect)) = rows
        .iter()
        .rev()
        .filter(|row| moving_id != Some(row.timing.id))
        .map(|row| (row, clip_rect(content, row, pixels_per_frame)))
        .find(|(_, rect)| rect.contains(pointer))
    {
        match drop_zone(rect, pointer.x) {
            ClipDropZone::Center(highlight) => {
                if moving_id
                    .is_none_or(|clip_id| !would_create_cycle(video, clip_id, row.timing.id))
                {
                    return Some(TimelineDropTarget::Attach {
                        parent: row.timing.id,
                        start: pointer_frame,
                        lane: row.lane + 1,
                        length: dragged_length,
                        highlight,
                    });
                }
            }
            ClipDropZone::Before if row.timing.depth == 0 => {
                let boundary = video.sibling_index(row.timing.id).unwrap_or(0);
                return Some(TimelineDropTarget::Base {
                    index: adjusted_base_index(video, boundary, moving_id),
                    x: rect.left(),
                });
            }
            ClipDropZone::After if row.timing.depth == 0 => {
                let boundary = video.sibling_index(row.timing.id).unwrap_or(0) + 1;
                return Some(TimelineDropTarget::Base {
                    index: adjusted_base_index(video, boundary, moving_id),
                    x: rect.right(),
                });
            }
            ClipDropZone::Before | ClipDropZone::After => {}
        }
    }

    let lane = lane_at(content, pointer.y);
    if let Some(drag) = moving {
        let row = rows.iter().find(|row| row.timing.id == drag.clip)?;
        let clip = video.clip(drag.clip)?;
        if lane == row.lane && clip.attachment.is_some() {
            return Some(TimelineDropTarget::Offset {
                start: moved_start,
                lane,
                length: clip.length,
            });
        }
    }
    (lane == 0).then(|| base_target(video, rows, content, pixels_per_frame, pointer.x, moving_id))
}

pub(crate) fn tick_seconds(frame_rate: VideoFrameRate, pixels_per_frame: f32) -> f64 {
    let pixels_per_second = frame_rate.frames_per_second() * f64::from(pixels_per_frame);
    TICK_SECONDS
        .into_iter()
        .find(|step| step * pixels_per_second >= MIN_TICK_SPACING)
        .unwrap_or(3600.0)
}

fn reattached(
    video: &Video,
    clip: &VideoClip,
    start: u64,
) -> Option<block_editor_beui::be_block::video::VideoClip> {
    let attachment = clip.attachment?;
    let parent_start = video.timing(attachment.clip_id)?.start;
    let offset =
        i64::try_from(start).unwrap_or(i64::MAX) - i64::try_from(parent_start).unwrap_or(0);
    (offset != attachment.offset).then(|| {
        let mut moved = clip.clone();
        moved.attachment = Some(VideoAttachment::new(attachment.clip_id, offset));
        moved
    })
}

pub(crate) fn apply_clip_drop(
    video: &Video,
    clip_id: Uuid,
    target: TimelineDropTarget,
    operations: &mut Vec<VideoOperation>,
) {
    let Some(clip) = video.clip(clip_id) else {
        return;
    };
    match target {
        TimelineDropTarget::Attach { parent, start, .. } => {
            let Some(parent_start) = video.timing(parent).map(|timing| timing.start) else {
                return;
            };
            let offset =
                i64::try_from(start).unwrap_or(i64::MAX) - i64::try_from(parent_start).unwrap_or(0);
            let attachment = Some(VideoAttachment::new(parent, offset));
            if clip.attachment != attachment {
                let mut attached = clip.clone();
                attached.attachment = attachment;
                operations.push(VideoOperation::UpdateClips {
                    clips: vec![attached],
                });
            }
            operations.push(VideoOperation::MoveClip { clip_id, index: 0 });
        }
        TimelineDropTarget::Base { index, .. } => {
            if clip.attachment.is_some() {
                let mut detached = clip.clone();
                detached.attachment = None;
                operations.push(VideoOperation::UpdateClips {
                    clips: vec![detached],
                });
            }
            operations.push(VideoOperation::MoveClip { clip_id, index });
        }
        TimelineDropTarget::Offset { start, .. } => {
            if let Some(update) = reattached(video, clip, start) {
                operations.push(VideoOperation::UpdateClips {
                    clips: vec![update],
                });
            }
        }
    }
}
