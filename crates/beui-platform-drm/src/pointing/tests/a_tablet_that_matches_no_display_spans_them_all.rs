use super::*;

#[test]
fn a_tablet_that_matches_no_display_spans_them_all() {
    let screens = [laptop(), pen_display(Rotation::Clockwise90)];
    let tablet = pointer(Kind::Pen, Some((152.0, 95.0)));
    let found = target(&tablet, &screens, None);

    assert_eq!(found, Target::All);
    assert_eq!(
        area(found, &areas(&screens)),
        (
            Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0 + 960.0, 720.0)),
            Rotation::Normal
        )
    );

    let alone = [pen_display(Rotation::Clockwise90)];
    assert_eq!(
        area(target(&tablet, &alone, None), &areas(&alone)),
        (alone[0].rect, Rotation::Clockwise90),
        "with one display, a tablet turns with it"
    );
}
