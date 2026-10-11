use super::*;

#[test]
fn a_frame_that_shows_nothing_waits_for_the_next_refresh() {
    let mut session = Session::new(App::Ticking, vec![Display::new(60)]);
    session.run_until(SECOND);
    assert_eq!(
        session.displays[0].flips, 1,
        "only the first frame changed anything"
    );
    assert!(
        (59..=61).contains(&session.frames),
        "frames that draw nothing still run only once a refresh, not {} times a second",
        session.frames
    );
}
