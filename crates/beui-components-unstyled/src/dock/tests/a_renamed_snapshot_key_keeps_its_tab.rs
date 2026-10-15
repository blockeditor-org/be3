use super::*;

#[test]
fn a_renamed_snapshot_key_keeps_its_tab() {
    let state = DockState::new([TabId::new(1), TabId::new(2)]);
    let mut snapshot = DockingSnapshot::new(
        state.clone(),
        [("first", TabId::new(1)), ("second", TabId::new(2))],
    );

    assert!(snapshot.rename(&"second", "renamed"));
    assert_eq!(
        snapshot.keys().copied().collect::<Vec<_>>(),
        vec!["first", "renamed"],
        "the key is renamed where it was"
    );
    assert_eq!(snapshot.state(), &state, "and the tabs stay as they were");
    assert!(
        !snapshot.rename(&"first", "renamed"),
        "a key is not renamed to one already held"
    );
    assert!(!snapshot.rename(&"missing", "other"));
}
