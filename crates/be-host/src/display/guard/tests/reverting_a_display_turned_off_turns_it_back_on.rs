use super::*;
use be_block::display_settings::ScreenUse;

#[test]
fn reverting_a_display_turned_off_turns_it_back_on() {
    let mut guard = showing(office_monitor(), saved(Some(slow())));
    let off = edited(
        &saved(Some(slow())),
        &[saved(Some(slow())).set_used(MONITOR, Some(ScreenUse::Off))],
    );
    assert!(!guard.take(off), "turning a display off waits to be kept");
    assert!(guard.asking());

    let changes = guard.revert();
    assert_eq!(
        changes,
        vec![Change::Use {
            id: MONITOR.to_owned(),
            used: None,
        }]
    );
    assert_eq!(guard.applied(), &saved(Some(slow())));
    assert!(!guard.asking());

    let layers = LayerContents {
        every_device: None,
        this_device: Some(saved(Some(slow()))),
    };
    let written = written(&layers, &changes);
    assert_eq!(written.len(), 1);
    assert_eq!(
        written[0].0,
        SettingsLayer::ThisDevice,
        "a display's use is kept on this device"
    );
}
