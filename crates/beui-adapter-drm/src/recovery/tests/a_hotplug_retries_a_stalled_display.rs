use super::*;

#[test]
fn a_hotplug_retries_a_stalled_display() {
    let mut card = FakeCard::default();
    let mut recovery = Recovery::default();
    recovery.failed(&mut card, 1, FLIP_FAILED.to_owned());
    recovery.failed(&mut card, 2, FLIP_FAILED.to_owned());
    recovery.failed(&mut card, 2, FLIP_FAILED.to_owned());
    card.take();

    recovery.changed(&mut card);
    assert_eq!(
        card.take(),
        [Call::Release(Some(2)), Call::Rescan],
        "only the stalled display is dropped, so the rescan builds it again"
    );

    recovery.failed(&mut card, 2, FLIP_FAILED.to_owned());
    assert_eq!(card.take(), [Call::Release(Some(2)), Call::Rescan]);
}
