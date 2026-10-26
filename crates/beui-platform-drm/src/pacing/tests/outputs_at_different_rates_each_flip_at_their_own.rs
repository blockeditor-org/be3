use super::*;

#[test]
fn outputs_at_different_rates_each_flip_at_their_own() {
    let mut session = Session::new(App::Animating, vec![Display::new(60), Display::new(144)]);
    session.run_until(SECOND);
    let (slow, fast) = (&session.displays[0], &session.displays[1]);
    assert!(
        (59..=61).contains(&slow.flips),
        "the 60 Hz display flipped {} times",
        slow.flips
    );
    assert!(
        (143..=145).contains(&fast.flips),
        "the 144 Hz display flipped {} times",
        fast.flips
    );
    assert!(
        session.frames <= slow.flips + fast.flips + 1,
        "{} frames ran for {} flips",
        session.frames,
        slow.flips + fast.flips
    );
}
