use super::*;

#[test]
fn a_failed_frame_sets_the_display_up_again_once_then_reports_once() {
    let mut card = FakeCard::default();
    let mut recovery = Recovery::default();

    recovery.failed(&mut card, 1, FLIP_FAILED.to_owned());
    assert_eq!(
        card.take(),
        [Call::Release(Some(1)), Call::Rescan],
        "the first failure sets the display up again, which modesets it"
    );

    recovery.failed(&mut card, 1, FLIP_FAILED.to_owned());
    assert_eq!(
        card.take(),
        [Call::Stall(1), Call::Report],
        "failing again stops the display drawing and tells the user once"
    );

    recovery.failed(&mut card, 1, FLIP_FAILED.to_owned());
    assert_eq!(card.take(), [Call::Stall(1)], "and does not tell them again");

    recovery.pause(&mut card);
    recovery.activate(&mut card);
    card.take();
    recovery.failed(&mut card, 1, FLIP_FAILED.to_owned());
    assert_eq!(
        card.take(),
        [Call::Release(Some(1)), Call::Rescan],
        "coming back to the terminal starts over"
    );

    recovery.shown(1);
    recovery.failed(&mut card, 1, FLIP_FAILED.to_owned());
    assert_eq!(
        card.take(),
        [Call::Release(Some(1)), Call::Rescan],
        "a frame that was shown starts over too"
    );
}
