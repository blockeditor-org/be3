use super::*;

#[test]
fn a_held_volume_key_keeps_turning_the_volume() {
    let mut fixture = Fixture::new();
    fixture.settle();
    assert!(
        fixture
            .test
            .hold_app_key(Modifiers::NONE, Key::VolumeDown, 3)
    );
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<MediaRequest>(),
        vec![MediaRequest::StepVolume(-VOLUME_STEP); 4],
        "the press and each of its repeats turn the volume down"
    );
}
