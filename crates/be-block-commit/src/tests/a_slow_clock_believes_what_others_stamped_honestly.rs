use super::*;

/// A device whose clock runs two hours slow believes commit times up to the server's clock plus
/// `MAX_COMMIT_AHEAD` once it measured that clock, so an honest edit stamped now is not taken for
/// a forgery; before a measurement, only its own clock.
#[test]
fn a_slow_clock_believes_what_others_stamped_honestly() {
    let server = 1_000 * HOUR;
    let slow = server - 2 * HOUR;
    assert_eq!(
        latest_believed(slow, Some(server)),
        server + MAX_COMMIT_AHEAD
    );
    assert_eq!(latest_believed(slow, None), slow + MAX_COMMIT_AHEAD);
    let fast = server + 2 * HOUR;
    assert_eq!(latest_believed(fast, Some(server)), fast + MAX_COMMIT_AHEAD);
}
