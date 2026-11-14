use super::*;

fn ms(milliseconds: u64) -> Duration {
    Duration::from_millis(milliseconds)
}

#[test]
fn a_frame_asked_for_while_one_is_in_flight_is_asked_again_once_it_is_answered() {
    let mut pacing = Pacing::default();
    pacing.request(ms(1000), 1);
    pacing.need();
    pacing.ready(ms(1000), None, false);
    assert!(
        pacing.due(ms(1000), 2),
        "what the app was told during its frame is drawn by the next"
    );

    pacing.request(ms(1000), 2);
    pacing.ready(ms(1000), None, false);
    assert!(
        !pacing.due(ms(1000), 3),
        "a need that was not made during a frame does not draw another"
    );

    pacing.need();
    assert!(
        pacing.due(ms(1000), 3),
        "a need with no frame in flight is due"
    );
    pacing.request(ms(1000), 3);
    pacing.ready(ms(1000), None, false);
    assert!(
        !pacing.due(ms(1000), 4),
        "and is met by the frame that drew it"
    );
}
