use super::*;

#[test]
fn a_frame_whose_input_is_still_waiting_answers_nothing_until_the_next() {
    let now = Duration::from_millis(1000);
    let mut pacing = Pacing::default();
    pacing.request(now, 1);
    pacing.ready(now, Some(Duration::ZERO), true);
    assert_eq!(
        pacing.answered, 0,
        "a frame that has not taken in all its input does not answer it"
    );
    pacing.request(now, 2);
    pacing.ready(now, None, false);
    assert_eq!(pacing.answered, 2, "the frame after it answers both");

    for pass in 3..3 + u64::from(INPUT_WAITING_FRAMES) {
        pacing.request(now, pass);
        pacing.ready(now, Some(Duration::ZERO), true);
        assert_eq!(pacing.answered, 2, "frame {pass} still waits");
    }
    pacing.request(now, 99);
    pacing.ready(now, Some(Duration::ZERO), true);
    assert_eq!(
        pacing.answered, pacing.asked,
        "an app that keeps saying so is answered after {INPUT_WAITING_FRAMES} frames all the same"
    );
}
