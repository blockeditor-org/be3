use uuid::Uuid;

use super::{Video, VideoEffect, VideoOperation, sample};
use block::Block;

#[test]
fn video_serialization_round_trips() {
    let (mut video, _, _, attached) = sample();
    Video::apply_operation(
        &mut video,
        &VideoOperation::SetFrameRate {
            frame_rate: super::VideoFrameRate::new(24, 1),
        },
    );

    let mut effected = video.clip(attached).unwrap().clone();
    effected.effects.push(VideoEffect {
        id: Uuid::new_v4(),
        name: "Placeholder".into(),
        enabled: true,
    });
    Video::apply_operation(
        &mut video,
        &VideoOperation::UpdateClips {
            clips: vec![effected],
        },
    );

    let json = serde_json::to_string(&video).unwrap();
    assert_eq!(serde_json::from_str::<Video>(&json).unwrap(), video);
}
