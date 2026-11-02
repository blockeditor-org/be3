use std::time::{Duration, Instant};

#[test]
fn a_real_clock_places_an_instant_ahead_of_now_that_far_ahead() {
    let clock = crate::Clock::real();
    let base = Instant::now();
    let gap = clock.at(base + Duration::from_secs(1)) - clock.at(base);
    assert!(
        gap.abs_diff(Duration::from_secs(1)) < Duration::from_millis(1),
        "an instant a second after another is placed a second after it, not {gap:?}"
    );
}
