use super::*;

#[test]
fn a_region_that_captures_touch_takes_fingers_landing_in_it() {
    let mut input = input(Vec::new(), vec![foot(true)]);

    assert!(
        !input.touch_down(1, pos2(400.0, 300.0)),
        "a finger outside every region is the program's"
    );
    assert!(!input.touch_moved(1, pos2(400.0, 290.0)));
    assert_eq!(input.state().touch_downs, 1, "but it is counted");
    assert_eq!(region(&input).touch_downs, 0);

    assert!(input.touch_down(2, pos2(400.0, 595.0)));
    assert_eq!(region(&input).touch, Some(pos2(400.0, 595.0)));
    assert!(
        input.touch_moved(2, pos2(410.0, 300.0)),
        "it keeps the finger wherever it goes"
    );
    assert_eq!(region(&input).touch, Some(pos2(410.0, 300.0)));
    assert!(input.touch_up(2, false));
    let state = region(&input);
    assert_eq!(
        (state.touch, state.lifts, state.lifted, state.touch_downs),
        (None, 1, Some(pos2(410.0, 300.0)), 1)
    );

    assert!(input.touch_down(3, pos2(400.0, 597.0)));
    assert!(input.touch_up(3, true));
    assert_eq!(
        (region(&input).lifts, region(&input).lifted),
        (2, None),
        "a cancelled touch lifts nowhere"
    );

    input.set(Vec::new(), vec![foot(false)]);
    assert!(
        !input.touch_down(4, pos2(400.0, 560.0)),
        "a region that does not capture lets fingers through"
    );
    assert_eq!(
        (region(&input).touch_downs, input.state().touch_downs),
        (3, 4),
        "and still counts them"
    );
}
