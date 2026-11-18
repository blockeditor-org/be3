use super::*;

const MILLISECOND: Duration = Duration::from_millis(1);
const REFRESH: Duration = Duration::from_micros(16_667);

#[test]
fn a_frame_waits_for_apps_until_the_next_present_less_its_own_composite() {
    let composites = VecDeque::from([3 * MILLISECOND, 5 * MILLISECOND, 2 * MILLISECOND]);
    let now = Duration::from_secs(5);
    assert_eq!(
        frame_wait(now, Some((now + REFRESH, REFRESH)), &composites),
        REFRESH - 6 * MILLISECOND,
        "the host keeps its longest recent composite and a millisecond to submit before the present"
    );
    assert_eq!(
        frame_wait(now, Some((now + MILLISECOND, REFRESH)), &composites),
        REFRESH + MILLISECOND - 6 * MILLISECOND,
        "a frame too late for this present waits for the one after it rather than not at all"
    );
    assert_eq!(
        frame_wait(now, None, &VecDeque::new()),
        UNKNOWN_REFRESH - UNMEASURED_COMPOSITE - SUBMIT_MARGIN,
        "a display that reports no refresh is taken as 60 Hz, and an unmeasured composite as 4 ms"
    );
}
