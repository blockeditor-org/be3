use super::*;

#[test]
fn a_key_with_nothing_to_change_says_so() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture
        .test
        .set_watched::<MediaWatch>(&MediaLevels::default());
    fixture.settle();

    assert!(fixture.test.host_key(Modifiers::NONE, Key::VolumeUp));
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.osd.0") && fixture.says("No audio device"),
        "a volume key with no output to turn up still answers"
    );
    fixture.test.snapshot("a_volume_key_with_no_audio_device");

    assert!(fixture.test.host_key(Modifiers::NONE, Key::MicMute));
    fixture.settle();
    assert!(fixture.says("No microphone"));

    assert!(fixture.test.host_key(Modifiers::NONE, Key::BrightnessDown));
    fixture.settle();
    assert!(fixture.says("No backlight"));
}
