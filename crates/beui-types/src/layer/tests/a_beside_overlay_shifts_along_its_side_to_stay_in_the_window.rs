use super::*;

#[test]
fn a_beside_overlay_shifts_along_its_side_to_stay_in_the_window() {
    let shown = placed(
        Placement::BELOW_START,
        rect(360.0, 120.0, 400.0, 150.0),
        60.0,
        40.0,
    );
    assert_eq!(
        shown.min,
        pos2(340.0, 150.0),
        "start-aligned at the right edge shifts left"
    );
    let shown = placed(
        Placement::BELOW_END,
        rect(0.0, 120.0, 40.0, 150.0),
        60.0,
        40.0,
    );
    assert_eq!(
        shown.min,
        pos2(0.0, 150.0),
        "end-aligned at the left edge shifts right"
    );
    let shown = placed(
        Placement::ABOVE_CENTER,
        rect(0.0, 120.0, 20.0, 150.0),
        60.0,
        40.0,
    );
    assert_eq!(
        shown.min,
        pos2(0.0, 80.0),
        "centred at the left edge shifts right"
    );
    let shown = placed(
        Placement::RIGHT_START,
        rect(150.0, 280.0, 250.0, 300.0),
        60.0,
        40.0,
    );
    assert_eq!(
        shown.min,
        pos2(250.0, 260.0),
        "start-aligned at the bottom shifts up"
    );
    let shown = placed(
        Placement::LEFT_END,
        rect(150.0, 0.0, 250.0, 20.0),
        60.0,
        40.0,
    );
    assert_eq!(
        shown.min,
        pos2(90.0, 0.0),
        "end-aligned at the top shifts down"
    );
    let shown = placed(
        Placement::RIGHT_CENTER,
        rect(150.0, 0.0, 250.0, 10.0),
        60.0,
        40.0,
    );
    assert_eq!(
        shown.min,
        pos2(250.0, 0.0),
        "centred at the top shifts down"
    );
}
