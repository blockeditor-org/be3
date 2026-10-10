use super::*;

#[test]
fn an_idle_output_draws_nothing() {
    let mut session = Session::new(App::Idle, vec![Display::new(60)]);
    session.run_until(SECOND);
    assert_eq!(session.frames, 1, "only the first frame runs");
    assert_eq!(
        session.displays[0].flips, 1,
        "only the first frame is shown"
    );
    assert_eq!(
        session.pacer.wait(session.now, &session.lanes()),
        Wait::Idle,
        "once its frame has flipped, nothing wakes the session"
    );

    session.pacer.ask();
    session.run_until(2 * SECOND);
    assert_eq!(session.frames, 2, "input runs one more frame");
    assert_eq!(
        session.displays[0].flips, 1,
        "a frame that changed nothing is not drawn"
    );
}
