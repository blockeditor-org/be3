use super::*;

#[test]
fn choosing_a_resolution_uses_its_fastest_refresh_rate() {
    let mut settings = Harness::in_a_session(vec![gaming_monitor()]);

    settings.choose(RESOLUTION, 1);
    assert_eq!(
        settings.content().root().mode(MONITOR),
        Some(saved(1920, 1080, 240_000))
    );

    settings.choose(REFRESH, 1);
    assert_eq!(
        settings.content().root().mode(MONITOR),
        Some(saved(1920, 1080, 60_000)),
        "the refresh rates offered are the chosen resolution's"
    );
}
