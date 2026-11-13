use super::*;

fn ms(milliseconds: u64) -> Duration {
    Duration::from_millis(milliseconds)
}

#[test]
fn an_app_that_asks_for_another_frame_at_once_without_animating_is_drawn_while_the_clock_stands_still()
 {
    let mut pacing = Pacing::default();
    pacing.request(ms(1000), 1);
    pacing.ready(ms(1000), Some(Duration::ZERO));
    assert!(
        pacing.owes_frame(),
        "a frame asked for at once with nothing animating is owed, so the host is busy until it is drawn"
    );
    assert_eq!(pacing.wake_after(ms(1000), true), Some(Duration::ZERO));
    assert!(
        pacing.due(ms(1000), 2),
        "work due now (a zero-delay timer) needs another frame but no time, so it is drawn while the \
         clock stands still"
    );
    pacing.request(ms(1000), 2);
    assert!(!pacing.owes_frame(), "the frame is in flight");
    assert!(!pacing.due(ms(1000), 2), "a frame is not asked for twice in one pass");

    pacing.ready(ms(1000), None);
    assert!(!pacing.owes_frame(), "the app asked for nothing more");
    assert!(!pacing.due(ms(1000), 3));
}
