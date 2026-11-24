use super::*;

fn ms(milliseconds: u64) -> Duration {
    Duration::from_millis(milliseconds)
}

#[test]
fn a_need_heard_before_a_present_nobody_asked_for_is_still_drawn() {
    // The app lays out and presents on its own when its surfaces are replaced, then asks for a
    // frame for what changed since: its `frame-needed` goes out at once, ahead of the present.
    let mut pacing = Pacing::default();
    pacing.need();
    pacing.presented(ms(1000), false, None, false);
    assert!(
        pacing.due(false, ms(1000), 1),
        "a present that answered no request meets no need"
    );
    pacing.request(ms(1000), 1);
    pacing.presented(ms(1000), false, None, false);
    assert!(
        !pacing.due(false, ms(1000), 2),
        "the frame asked for meets it"
    );
}
