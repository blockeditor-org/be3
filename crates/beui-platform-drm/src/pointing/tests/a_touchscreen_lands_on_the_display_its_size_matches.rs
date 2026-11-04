use super::*;

#[test]
fn a_touchscreen_lands_on_the_display_its_size_matches() {
    let mut big = pen_display(Rotation::Normal);
    big.id = "touch-monitor".to_owned();
    big.size = Some((598, 336));
    let screens = [laptop(), big];

    let external = pointer(Kind::Touch, Some((600.0, 340.0)));
    assert_eq!(target(&external, &screens, None), Target::Output(1));

    let held_upright = pointer(Kind::Touch, Some((336.0, 598.0)));
    assert_eq!(
        target(&held_upright, &screens, None),
        Target::Output(1),
        "a digitizer that reports its sides the other way round still matches"
    );

    assert_eq!(
        place(pos2(0.5, 0.5), Target::Output(1), &areas(&screens)),
        pos2(1280.0 + 480.0, 270.0),
        "a touch lands on its display's points, whatever that display's scale"
    );
}
