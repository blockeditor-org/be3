use super::*;

fn ms(milliseconds: u64) -> Duration {
    Duration::from_millis(milliseconds)
}

#[test]
fn an_app_animating_while_the_clock_stands_still_is_drawn_again_only_once_time_moves() {
    let mut pacing = Pacing::default();
    pacing.request(ms(1000), 1);
    pacing.presented(ms(1000), true, Some(Duration::ZERO), false);
    assert!(
        !pacing.due(false, ms(1000), 2),
        "an animation asking for its next frame at once is not drawn again while the clock stands still"
    );
    assert!(!pacing.due(false, ms(1000), 3));

    assert!(
        pacing.due(false, ms(1016), 4),
        "once time moves the app draws"
    );
    pacing.request(ms(1016), 4);
    pacing.presented(ms(1016), true, Some(Duration::ZERO), false);
    assert!(!pacing.due(false, ms(1016), 5));

    pacing.needed = true;
    assert!(
        pacing.due(false, ms(1016), 5),
        "a change still reaches the app while the clock stands still"
    );
}
