use be_block::display_settings::Rotation;

use super::*;

#[test]
fn a_committed_mode_keeps_the_rotation_of_the_layer_under_it() {
    let store = Store::open();
    let mut lower = Document::new(&DisplaySettings::default());
    lower.apply(&DisplaySettings::default().set_rotation(MONITOR, Some(Rotation::Clockwise90)));
    let mut sync = SettingsSync::<DisplaySettings> {
        contents: Some(vec![lower.root(), DisplaySettings::default()]),
        ..SettingsSync::default()
    };
    sync.receive(&store.store, lower.root());

    let edit = sync.top().set_mode(MONITOR, Some(SLOW));
    sync.commit(&store.store, vec![edit]);

    let applied = sync.applied().expect("settings apply").clone();
    assert_eq!(applied.mode(MONITOR), Some(SLOW));
    assert_eq!(
        applied.rotation(MONITOR),
        Rotation::Clockwise90,
        "the edit to the top layer leaves the rotation it inherits"
    );
    assert_eq!(store.saved(), Some(applied));
    assert_eq!(sync.top().screen(MONITOR).rotation, None);
}
