use super::*;
use crate::video::{VideoClip, VideoContent, VideoOperation};

fn run(content: &VideoContent, operation: VideoOperation) -> VideoContent {
    edited(content, [content.root().edit_for(&operation)])
}

fn attach(content: &VideoContent, clip: Uuid, to: Uuid) -> VideoOperation {
    let clip = content
        .root()
        .video()
        .clip(clip)
        .expect("the clip is in the video")
        .clone()
        .attached_to(to, 1);
    VideoOperation::UpdateClips { clips: vec![clip] }
}

#[test]
fn clips_attached_to_each_other_on_each_side_merge_without_a_cycle() {
    let first = VideoClip::new(Uuid::new_v4(), 10);
    let second = VideoClip::new(Uuid::new_v4(), 5);
    let (first_id, second_id) = (first.id, second.id);
    let mut base = VideoContent::default();
    for (index, clip) in [first, second].into_iter().enumerate() {
        base = run(&base, VideoOperation::InsertClip { clip, index });
    }
    let ours = run(&base, attach(&base, first_id, second_id));
    let theirs = run(&base, attach(&base, second_id, first_id));

    let (merged, conflicts) = merged(&base, &ours, &theirs);
    let sequenced = edited(
        &base,
        [
            base.root().edit_for(&attach(&base, first_id, second_id)),
            base.root().edit_for(&attach(&base, second_id, first_id)),
        ],
    );

    assert!(conflicts >= 1);
    for content in [merged, sequenced] {
        let video = content.root().video();
        let mut clips: Vec<Uuid> = video.clips().iter().map(|clip| clip.id).collect();
        clips.sort();
        let mut expected = vec![first_id, second_id];
        expected.sort();
        assert_eq!(clips, expected);
        assert_eq!(
            video.children(None).len(),
            1,
            "exactly one clip is at the top"
        );
    }
}
