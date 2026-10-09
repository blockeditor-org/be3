use super::*;

#[test]
fn taking_the_seat_back_rebuilds_every_display_from_scratch() {
    let mut card = FakeCard::default();
    let mut recovery = Recovery::default();
    assert!(recovery.active());

    recovery.pause(&mut card);
    assert!(!recovery.active());
    assert_eq!(card.take(), [Call::Suspend]);

    recovery.pause(&mut card);
    assert_eq!(card.take(), [], "a second pause has nothing left to suspend");

    recovery.activate(&mut card);
    assert!(recovery.active());
    assert_eq!(
        card.take(),
        [Call::Release(None), Call::TakeBack, Call::Rescan],
        "every output is dropped while the device is still paused, so dropping it commits \
         nothing, then the device is reset and the outputs are built for what is connected now"
    );
}
