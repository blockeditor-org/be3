use super::*;

fn ms(milliseconds: u64) -> Duration {
    Duration::from_millis(milliseconds)
}

#[test]
fn a_plugin_animating_while_the_clock_stands_still_is_drawn_again_only_once_time_moves() {
    let mut pacing = Pacing::default();
    pacing.request(ms(1000), 1);
    pacing.ready(ms(1000), Some(Duration::ZERO));
    assert!(
        !pacing.due(ms(1000), 2),
        "a frame asked for at once is not drawn again while the clock stands still"
    );
    assert!(!pacing.due(ms(1000), 3));

    assert!(pacing.due(ms(1016), 4), "once time moves the plugin draws");
    pacing.request(ms(1016), 4);
    pacing.ready(ms(1016), Some(Duration::ZERO));
    assert!(!pacing.due(ms(1016), 5));

    pacing.needed = true;
    assert!(
        pacing.due(ms(1016), 5),
        "a change still reaches the plugin while the clock stands still"
    );
}
