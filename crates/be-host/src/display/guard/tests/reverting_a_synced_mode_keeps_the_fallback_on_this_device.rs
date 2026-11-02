use super::*;

#[test]
fn reverting_a_synced_mode_keeps_the_fallback_on_this_device() {
    let mut guard = Guard::default();
    guard.take(saved(Some(small())));
    guard.set_screens(vec![gaming_monitor()]);

    let changes = guard.revert();
    assert_eq!(
        changes,
        vec![ModeChange {
            here: true,
            ..change(Some(slow()))
        }]
    );
    assert!(!guard.asking());

    let layers = LayerContents {
        every_device: Some(saved(Some(small()))),
        this_device: Some(DisplaySettings::default()),
    };
    let written = written(&layers, &changes);
    assert_eq!(written.len(), 1);
    assert_eq!(
        written[0].0,
        SettingsLayer::ThisDevice,
        "the other devices keep the mode they chose"
    );
}
