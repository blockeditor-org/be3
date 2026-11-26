use super::*;

fn ms(milliseconds: u64) -> Duration {
    Duration::from_millis(milliseconds)
}

#[test]
fn a_frame_asked_for_at_once_without_animating_is_drawn_in_the_next_host_frame_while_the_clock_stands_still()
 {
    let mut pacing = Pacing::default();
    pacing.request(ms(1000), 1);
    pacing.presented(ms(1000), false, Some(Duration::ZERO), false);
    assert!(
        pacing.due(false, ms(1000), 2),
        "one more frame asked for at once is drawn in the next host frame, though the clock stands still"
    );
    pacing.request(ms(1000), 2);
    pacing.presented(ms(1000), false, None, false);
    assert!(!pacing.due(false, ms(1000), 3), "and no more after it");
}
