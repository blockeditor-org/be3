use super::*;

#[test]
fn a_continuous_animation_draws_one_frame_per_flip() {
    let mut session = Session::new(App::Animating, vec![Display::new(240)]);
    session.run_until(SECOND);
    let display = &session.displays[0];
    assert!(
        (239..=241).contains(&display.flips),
        "the display flips at its refresh rate, not {} times a second",
        display.flips
    );
    assert!(
        session.frames <= display.flips + 1,
        "{} frames ran for {} flips",
        session.frames,
        display.flips
    );
    assert_eq!(
        display.drawn, session.frames,
        "every frame that ran was drawn"
    );
}
