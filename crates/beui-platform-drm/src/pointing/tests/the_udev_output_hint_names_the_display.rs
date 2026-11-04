use super::*;

#[test]
fn the_udev_output_hint_names_the_display() {
    let screens = [laptop(), pen_display(Rotation::Normal)];
    let mut touch = pointer(Kind::Touch, Some((310.0, 174.0)));
    touch.output = Some("HDMI-A-1".to_owned());

    assert_eq!(
        target(&touch, &screens, None),
        Target::Output(1),
        "WL_OUTPUT wins over the size the panel matches"
    );

    touch.output = Some("DP-3".to_owned());
    assert_eq!(
        target(&touch, &screens, None),
        Target::Output(0),
        "a hint naming no connected display is ignored"
    );
}
