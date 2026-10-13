use super::*;

#[test]
fn the_default_is_the_preferred_size_at_its_fastest_refresh() {
    let candidates = gaming_monitor();
    assert_eq!(default_mode(&candidates), Some(1));
    assert_eq!(choose(&candidates, None), Some(1));

    let unpreferred = vec![candidate(1280, 720, 60_000), candidate(1920, 1080, 50_000)];
    assert_eq!(
        default_mode(&unpreferred),
        Some(1),
        "without a preferred mode the largest one is used"
    );

    assert_eq!(default_mode(&[]), None);
}
