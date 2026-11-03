use super::*;

#[test]
fn an_integrated_touchscreen_lands_on_the_built_in_panel() {
    let screens = [pen_display(Rotation::Normal), laptop()];

    let no_size = pointer(Kind::Touch, None);
    assert_eq!(
        target(&no_size, &screens, None),
        Target::Output(1),
        "with no size to match, a touchscreen is the built-in panel's even when it is not first"
    );

    let external = [pen_display(Rotation::Normal), {
        let mut other = pen_display(Rotation::Normal);
        other.id = "other".to_owned();
        other.connector = "DP-1".to_owned();
        other.size = None;
        other
    }];
    assert_eq!(
        target(&no_size, &external, None),
        Target::Output(0),
        "without a built-in panel, the first display"
    );

    let mut twins = [laptop(), pen_display(Rotation::Normal)];
    twins[1].size = twins[0].size;
    assert_eq!(
        target(&pointer(Kind::Touch, Some((310.0, 174.0))), &twins, None),
        Target::Output(0),
        "two displays of the same size go to the built-in one"
    );
}
