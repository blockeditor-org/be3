use std::time::Duration;

use super::*;

#[test]
fn what_is_sent_asks_for_a_frame_and_waits_for_the_session() {
    let mut run = Run::<()>::new(Session::new("Test", None), true, Preferences::default());
    run.session.start(0);
    let due = |run: &Run<()>| run.pacing.will_draw(false, Duration::ZERO, 1);
    assert!(run.send_for_frame(Vec::new(), Some).is_empty());
    assert!(!due(&run), "nothing sent asks for no frame");

    let sent = run.send_for_frame(vec![Outbound::App(AppCall::Paintings)], Some);
    assert!(
        sent.is_empty(),
        "a session that has not connected is sent nothing yet"
    );
    assert!(due(&run), "what is sent asks for a frame");
    let dropped = run.send_for_frame(vec![Outbound::App(AppCall::Describe(true))], |_| None);
    assert!(dropped.is_empty(), "what the host drops is never queued");

    connect(&mut run);
    assert!(
        matches!(
            run.deliverable().as_slice(),
            [Outbound::App(AppCall::Paintings)]
        ),
        "once it runs, what was queued goes"
    );
    assert!(
        matches!(
            run.queue([Outbound::App(AppCall::Describe(false))])
                .as_slice(),
            [Outbound::App(AppCall::Describe(false))]
        ),
        "and what is queued after goes at once"
    );
}
