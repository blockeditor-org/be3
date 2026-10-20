use super::*;

#[test]
fn a_reverted_display_mode_is_never_saved() {
    let store = Store::open();
    let mut sync = SettingsSync::<DisplaySettings>::default();
    sync.receive(&store.store, showing(SLOW));
    connect_monitor();

    sync.receive(&store.store, showing(FAST));
    assert!(display::asking());

    let edits = display::revert(&store.store, &sync.layers());
    sync.commit(&store.store, edits);
    assert!(!display::asking());
    assert_eq!(store.saved(), Some(showing(SLOW)));
    assert_eq!(
        sync.outboxes[0].unwritten,
        vec![DisplaySettings::default().set_mode(MONITOR, Some(SLOW))],
        "the block is put back once it can be written"
    );
}
