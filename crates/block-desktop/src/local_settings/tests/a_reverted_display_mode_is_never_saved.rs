use super::*;

#[test]
fn a_reverted_display_mode_is_never_saved() {
    let store = Store::default();
    let mut sync = SettingsSync::<DisplaySettings>::default();
    sync.receive(&store, showing(SLOW));
    connect_monitor();

    sync.receive(&store, showing(FAST));
    assert!(display::asking());

    sync.commit(&store, display::revert());
    assert!(!display::asking());
    assert_eq!(store.saved(), Some(showing(SLOW)));
    assert_eq!(
        sync.outbox.unwritten,
        vec![DisplaySettings::set_mode(MONITOR, Some(SLOW))],
        "the block is put back once it can be written"
    );
}
