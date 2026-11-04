use super::*;

#[test]
fn choosing_a_refresh_rate_stores_the_mode() {
    let mut settings = Harness::in_a_session(vec![gaming_monitor()]);
    settings
        .editor
        .snapshot("choosing_a_refresh_rate_stores_the_mode");
    assert_eq!(settings.content().root().mode(MONITOR), None);

    settings.choose(REFRESH, 1);

    assert_eq!(
        settings.content().root().mode(MONITOR),
        Some(saved(2560, 1440, 143_998))
    );
}
