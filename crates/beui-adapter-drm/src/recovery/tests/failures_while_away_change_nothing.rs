use super::*;

#[test]
fn failures_while_away_change_nothing() {
    let mut card = FakeCard::default();
    let mut recovery = Recovery::default();
    recovery.pause(&mut card);
    card.take();

    recovery.failed(&mut card, 1, FLIP_FAILED.to_owned());
    recovery.changed(&mut card);
    assert_eq!(
        card.take(),
        [],
        "a frame whose fence signals while another terminal has the seat cannot be shown, and \
         a hotplug then waits for the seat to come back, which rescans anyway"
    );

    recovery.activate(&mut card);
    card.take();
    recovery.failed(&mut card, 1, FLIP_FAILED.to_owned());
    assert_eq!(card.take(), [Call::Release(Some(1)), Call::Rescan]);
}
