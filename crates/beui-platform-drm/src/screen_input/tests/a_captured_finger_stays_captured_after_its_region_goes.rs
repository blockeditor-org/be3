use super::*;

#[test]
fn a_captured_finger_stays_captured_after_its_region_goes() {
    let mut input = input(Vec::new(), vec![foot(true)]);
    assert!(input.touch_down(1, pos2(400.0, 595.0)));

    input.set(Vec::new(), Vec::new());
    assert!(
        input.touch_moved(1, pos2(400.0, 500.0)),
        "the rest of the touch is not handed to a window that never saw it start"
    );
    assert!(input.touch_up(1, false), "nor is its lift");

    assert!(
        !input.touch_moved(1, pos2(400.0, 480.0)),
        "once lifted it is released"
    );
    assert!(
        !input.touch_down(2, pos2(400.0, 595.0)),
        "and with the region gone a new finger is the program's"
    );
}
