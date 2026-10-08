use super::*;

#[test]
fn a_held_volume_key_keeps_turning_the_volume() {
    let mut harness = Harness::new();
    harness.frame(vec![key(Key::VolumeDown, true, false)]);
    for _ in 0..3 {
        harness.frame(vec![key(Key::VolumeDown, true, true)]);
    }
    harness.frame(vec![key(Key::VolumeDown, false, false)]);
    assert_eq!(
        harness.sent(),
        vec![Sent::Audio(AudioRequest::Volume(-VOLUME_STEP)); 4],
        "the press and each of its repeats turn the volume down"
    );
}
