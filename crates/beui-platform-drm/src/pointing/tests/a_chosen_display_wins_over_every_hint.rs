use super::*;

#[test]
fn a_chosen_display_wins_over_every_hint() {
    let screens = [laptop(), pen_display(Rotation::Normal)];
    let mut touch = pointer(Kind::Touch, Some((310.0, 174.0)));
    touch.output = Some("eDP-1".to_owned());

    assert_eq!(
        target(&touch, &screens, Some("WAC|Cintiq 16|1234")),
        Target::Output(1)
    );
    assert_eq!(
        target(&touch, &screens, Some("HDMI-A-1")),
        Target::Output(1),
        "a display without an EDID is known by its connector"
    );
    assert_eq!(
        target(&touch, &screens, Some("unplugged")),
        Target::Output(0),
        "a chosen display that is not connected leaves the device where it goes by itself"
    );
}
