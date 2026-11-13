use super::*;

/// The Linux host applies `settings()` to its compositor when it starts, so what be-host
/// configured before then (display and input settings load first) is not lost.
#[test]
fn a_setting_made_before_the_platform_starts_is_kept_for_it() {
    let display = be_block::DisplaySettings::default();
    assert_eq!(settings().blank_after, display.screen_off().after());
    assert_eq!(settings().lock_after, display.lock_time());

    configure(|desktop| desktop.primary_selection = true);
    configure(|desktop| desktop.blank_after = Some(Duration::from_secs(90)));
    assert_eq!(
        settings(),
        DesktopSettings {
            primary_selection: true,
            blank_after: Some(Duration::from_secs(90)),
            ..DesktopSettings::default()
        },
        "each change keeps the others"
    );
}
