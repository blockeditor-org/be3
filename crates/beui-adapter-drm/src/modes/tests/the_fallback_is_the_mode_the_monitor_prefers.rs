use super::*;

#[test]
fn the_fallback_is_the_mode_the_monitor_prefers() {
    let candidates = gaming_monitor();
    assert_eq!(
        preferred_mode(&candidates),
        Some(0),
        "the monitor's own preferred mode, not the fastest at its size"
    );

    let unpreferred = vec![candidate(1280, 720, 60_000), candidate(1920, 1080, 50_000)];
    assert_eq!(
        preferred_mode(&unpreferred),
        default_mode(&unpreferred),
        "without a preferred mode the fallback is the default"
    );
}
