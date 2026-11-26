use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::*;

#[test]
fn a_runtime_with_no_audio_output_says_so_when_asked_to_play() {
    let changes = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&changes);
    let mut player = open(Box::new(move || {
        counted.fetch_add(1, Ordering::Relaxed);
    }));

    player.toggle();
    assert_eq!(player.status().error, None, "nothing is loaded to play");

    player.load(AudioSource {
        media_type: "audio/wav".into(),
        bytes: vec![0; 4],
    });
    assert_eq!(player.status().error, None);
    player.toggle();
    let status = player.status();
    assert!(!status.playing);
    assert_eq!(
        status.error.as_deref(),
        Some("This host has no audio output")
    );

    player.reset();
    assert_eq!(player.status().error, None);
    assert_eq!(changes.load(Ordering::Relaxed), 4, "each command is heard");
}
