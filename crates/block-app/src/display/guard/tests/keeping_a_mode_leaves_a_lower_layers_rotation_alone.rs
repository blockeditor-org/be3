use super::*;

#[test]
fn keeping_a_mode_leaves_a_lower_layers_rotation_alone() {
    let turned = edited(
        &DisplaySettings::default(),
        &[DisplaySettings::default().set_rotation(MONITOR, Some(Rotation::Clockwise270))],
    );
    let mut guard = showing(gaming_monitor(), turned);
    assert!(guard.asking());

    let changes = guard.keep();
    let layers = LayerContents {
        every_device: Some(DisplaySettings::default()),
        this_device: None,
    };
    let edits: Vec<Edit> = written(&layers, &changes)
        .into_iter()
        .map(|(layer, edit)| {
            assert_eq!(layer, SettingsLayer::EveryDevice);
            edit
        })
        .collect();
    let kept = edited(&DisplaySettings::default(), &edits);

    assert_eq!(kept.mode(MONITOR), Some(fast()));
    assert_eq!(
        kept.screen(MONITOR).rotation,
        None,
        "the layer written is not handed the rotation it inherits"
    );
    assert_eq!(guard.applied().rotation(MONITOR), Rotation::Clockwise270);
    assert_eq!(guard.applied().mode(MONITOR), Some(fast()));
}
