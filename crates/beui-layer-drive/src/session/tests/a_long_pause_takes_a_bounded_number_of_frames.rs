use super::*;

fn frames(pause: Duration) -> usize {
    let mut remaining = pause;
    let mut frames = 0;
    while !remaining.is_zero() {
        remaining -= pause_step(remaining);
        frames += 1;
    }
    frames
}

#[test]
fn a_long_pause_takes_a_bounded_number_of_frames() {
    assert_eq!(frames(Duration::from_millis(600)), 36, "a short pause runs a frame interval at a time");
    assert!(
        frames(Duration::from_secs(60)) < 500,
        "a minute is not 3600 frames: {}",
        frames(Duration::from_secs(60))
    );
    assert_eq!(pause_step(Duration::from_millis(5)), Duration::from_millis(5));
}
