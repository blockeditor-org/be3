use super::*;

#[test]
fn a_turned_screen_is_not_asked_about() {
    let mut guard = showing(gaming_monitor(), saved(Some(slow())));
    let turned = edited(
        &saved(Some(slow())),
        &[saved(Some(slow())).set_rotation(MONITOR, Some(Rotation::Clockwise90))],
    );

    assert!(guard.take(turned), "a rotation applies at once");
    assert!(!guard.asking());
    assert_eq!(guard.applied().rotation(MONITOR), Rotation::Clockwise90);
}
