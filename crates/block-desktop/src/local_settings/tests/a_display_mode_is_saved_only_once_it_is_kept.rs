use super::*;

#[test]
fn a_display_mode_is_saved_only_once_it_is_kept() {
    let store = Store::default();
    let mut sync = SettingsSync::<DisplaySettings>::default();
    sync.receive(&store, showing(SLOW));
    connect_monitor();
    assert!(!display::asking());

    sync.receive(&store, showing(FAST));
    assert!(display::asking(), "the new mode asks to be kept");
    assert_eq!(
        store.saved(),
        Some(showing(SLOW)),
        "the new mode is not saved yet"
    );

    sync.commit(&store, display::keep());
    assert!(!display::asking());
    assert_eq!(store.saved(), Some(showing(FAST)), "keeping saves it");
}
