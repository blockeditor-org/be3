use super::*;

#[test]
fn a_child_takes_the_presses_only_where_it_shows() {
    // A child placed at 20..220 across, clipped to its placer's view at 150, in a placer drawn
    // at two pixels a point and shown up to its pixel 360: the hole it cuts in its placer's
    // hits is the part of it that shows, in the placer's pixels.
    let mut placement = placed(rect((20.0, 10.0), (220.0, 60.0)), ChildMode::Live);
    placement.clip = rect((0.0, 0.0), (150.0, 400.0));
    let placer_clip = rect((0.0, 0.0), (360.0, 800.0));
    assert_eq!(
        hole(&placement, 2.0, placer_clip),
        rect((40.0, 20.0), (300.0, 120.0))
    );
    let shown_less = rect((0.0, 0.0), (100.0, 800.0));
    assert_eq!(
        hole(&placement, 2.0, shown_less),
        rect((40.0, 20.0), (100.0, 120.0)),
        "nor where its placer is clipped away"
    );
}
