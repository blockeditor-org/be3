use super::*;

#[test]
fn a_pen_display_lands_on_its_own_screen_turned_with_it() {
    let screens = [laptop(), pen_display(Rotation::Clockwise90)];
    let pen = pointer(Kind::Pen, Some((345.0, 195.0)));
    let found = target(&pen, &screens, None);

    assert_eq!(found, Target::Output(1));
    assert_eq!(
        place(pos2(0.0, 0.0), found, &areas(&screens)),
        pos2(1280.0, 540.0),
        "the panel's top left is the picture's bottom left once turned a quarter clockwise"
    );
    assert_eq!(
        place(pos2(1.0, 1.0), found, &areas(&screens)),
        pos2(1280.0 + 960.0, 0.0)
    );
    assert_eq!(
        area(found, &areas(&screens)),
        (screens[1].rect, Rotation::Clockwise90),
        "the pen's tilt turns with the screen it lands on"
    );
}
