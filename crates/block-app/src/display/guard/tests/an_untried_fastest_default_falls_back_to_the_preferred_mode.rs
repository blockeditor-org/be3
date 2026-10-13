use super::*;

#[test]
fn an_untried_fastest_default_falls_back_to_the_preferred_mode() {
    let mut guard = showing(gaming_monitor(), DisplaySettings::default());
    assert!(
        guard.asking(),
        "a monitor started at a fastest rate it never showed asks to keep it"
    );

    assert_eq!(
        guard.revert(),
        vec![DisplaySettings::set_mode(MONITOR, Some(slow()))]
    );
    assert_eq!(guard.applied().mode(MONITOR), Some(slow()));
    assert!(!guard.asking(), "the fallback is remembered");
}
