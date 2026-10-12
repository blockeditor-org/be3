use super::*;

#[test]
fn a_flip_from_before_a_screen_was_drawn_again_is_not_reported() {
    let flips = Flips::default();
    flips.shown(flip("left", 1));
    flips.shown(flip("right", 1));
    flips.drawn("left");
    assert_eq!(
        flips.take(),
        vec![flip("right", 1)],
        "the left flip showed an older frame than the one just drawn"
    );

    flips.shown(flip("left", 2));
    flips.shown(flip("left", 3));
    assert_eq!(
        flips.take(),
        vec![flip("left", 3)],
        "only a screen's latest flip is kept"
    );
}
