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
    let top = DisplaySettings::default();
    let kept = edited(&top, &written(&top, &changes));

    assert_eq!(kept.mode(MONITOR), Some(fast()));
    assert_eq!(
        kept.screen(MONITOR).rotation,
        None,
        "the top layer is not handed the rotation it inherits"
    );
    assert_eq!(guard.applied().rotation(MONITOR), Rotation::Clockwise270);
    assert_eq!(guard.applied().mode(MONITOR), Some(fast()));
}
