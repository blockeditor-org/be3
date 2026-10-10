use super::*;

#[test]
fn other_displays_keep_their_own_streak() {
    let mut card = FakeCard::default();
    let mut recovery = Recovery::default();

    recovery.failed(&mut card, 1, FLIP_FAILED.to_owned());
    recovery.shown(2);
    recovery.failed(&mut card, 1, FLIP_FAILED.to_owned());
    assert_eq!(
        card.take(),
        [
            Call::Release(Some(1)),
            Call::Rescan,
            Call::Stall(1),
            Call::Report
        ],
        "a healthy display showing frames does not let a broken one be rebuilt every frame"
    );
}
