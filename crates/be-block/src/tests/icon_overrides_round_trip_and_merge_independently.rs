use crate::metadata::{BlockMetadata, ICON, ICON_COLOR};

#[test]
fn icon_overrides_round_trip_and_merge_independently() {
    let base = BlockMetadata::named("Notes");
    let mut ours = base.clone();
    ours.insert(&ICON, &'\u{e838}');
    let mut theirs = base.clone();
    theirs.insert(&ICON_COLOR, &[65, 145, 210]);
    let (defined, conflicts) = BlockMetadata::merge_defined(&base, &ours, &theirs);
    assert!(conflicts.is_empty());
    let merged = base.with_defined(defined);
    let decoded = BlockMetadata::decode(&merged.encode()).unwrap();
    assert_eq!(decoded.get(&ICON), Some('\u{e838}'));
    assert_eq!(decoded.get(&ICON_COLOR), Some([65, 145, 210]));
    assert_eq!(decoded.name().as_deref(), Some("Notes"));
}
