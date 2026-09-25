use super::*;
use crate::video::{
    MAX_CLIP_LENGTH, Video, VideoAttachment, VideoClip, VideoContent, VideoFrameRate,
    VideoOperation,
};
use crate::{ChildChange, Root};
use uuid::Uuid;

struct Sample {
    content: VideoContent,
}

impl Sample {
    fn video(&self) -> Video {
        self.content.root().video()
    }

    fn run(&mut self, operation: VideoOperation) {
        let edit = self.content.root().edit_for(&operation);
        self.content.apply(&edit);
    }

    fn update(&mut self, id: Uuid, change: impl FnOnce(&mut VideoClip)) {
        let mut clip = self.video().clip(id).unwrap().clone();
        change(&mut clip);
        self.run(VideoOperation::UpdateClips { clips: vec![clip] });
    }

    fn starts(&self) -> Vec<(Uuid, u64, usize)> {
        self.video()
            .timeline()
            .iter()
            .map(|timing| (timing.id, timing.start, timing.depth))
            .collect()
    }
}

fn sample() -> (Sample, Uuid, Uuid, Uuid) {
    let mut sample = Sample {
        content: VideoContent::default(),
    };
    let first = VideoClip::new(Uuid::new_v4(), 10);
    let second = VideoClip::new(Uuid::new_v4(), 5);
    let attached = VideoClip::new(Uuid::new_v4(), 3).attached_to(first.id, 2);
    let ids = (first.id, second.id, attached.id);
    for (index, clip) in [first, second, attached].into_iter().enumerate() {
        sample.run(VideoOperation::InsertClip { clip, index });
    }
    (sample, ids.0, ids.1, ids.2)
}

#[test]
fn video_clips_attach_ripple_and_refuse_cycles() {
    let (mut video, first, second, attached) = sample();
    assert_eq!(
        video.starts(),
        [(first, 0, 0), (attached, 2, 1), (second, 10, 0)]
    );
    assert_eq!(video.video().frame_rate(), VideoFrameRate::DEFAULT);

    video.update(first, |clip| {
        clip.attachment = Some(VideoAttachment::new(attached, 0))
    });
    assert_eq!(video.video().clip(first).unwrap().attachment, None);
    video.update(attached, |clip| {
        clip.attachment = Some(VideoAttachment::new(attached, 4));
    });
    assert_eq!(
        video.video().clip(attached).unwrap().attachment,
        Some(VideoAttachment::new(first, 2))
    );
    video.update(attached, |clip| {
        clip.attachment = Some(VideoAttachment::new(second, 1));
    });
    assert_eq!(video.video().timing(attached).unwrap().start, 11);
    video.update(attached, |clip| {
        clip.attachment = Some(VideoAttachment::new(first, 2))
    });

    video.update(first, |clip| clip.length = u64::MAX);
    assert_eq!(video.video().clip(first).unwrap().length, MAX_CLIP_LENGTH);
    video.run(VideoOperation::SetFrameRate {
        frame_rate: VideoFrameRate::new(0, 0),
    });
    assert_eq!(video.video().frame_rate(), VideoFrameRate::new(1, 1));

    video.run(VideoOperation::MoveClip {
        clip_id: second,
        index: 0,
    });
    assert_eq!(video.video().children(None)[0].id, second);

    video.run(VideoOperation::RemoveClips { ids: vec![first] });
    assert_eq!(video.starts(), [(second, 0, 0)]);
    assert!(video.video().clip(attached).is_none());

    let block = video.video().clip(second).unwrap().block_id;
    let replacement = Uuid::new_v4();
    let edit = video
        .content
        .root()
        .child_edit(ChildChange::Replace {
            old: block,
            new: replacement,
        })
        .unwrap();
    video.content.apply(&edit);
    assert_eq!(BlockContent::references(&video.content), [replacement]);
}
