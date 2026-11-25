use super::*;

#[test]
fn a_window_is_kept_within_its_openers_work_area_but_while_it_moves() {
    // Its opener keeps a bar 40 points tall along its bottom: a window opened with its grip
    // reaching into it is brought up until its grip is clear of it, as be-host's desktop keeps
    // one (the rest may hang over the bar), unless it is the one being moved.
    let mut windows = Vec::new();
    let _ = hear(
        &mut windows,
        &[main_screen(true)],
        vec![open(7, 1, pos2(40.0, 540.0))],
        None,
        Pos2::ZERO,
    );
    let work = |_: ViewRef| Some(Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 560.0)));
    assert!(!keep_in_work_areas(&mut windows, Some(ViewId(7)), work));
    assert_eq!(windows[0].rect.max.y, 640.0, "left alone while it moves");
    assert!(keep_in_work_areas(&mut windows, None, work));
    assert_eq!(
        windows[0].rect.min,
        pos2(40.0, 560.0 - be_sdk_window_stack::KEPT.y),
        "its grip kept clear of the bar"
    );
    // It is asked for where it is wanted, which it goes back to once there is room.
    assert_eq!(
        windows[0].request().host_place,
        Some(be_sdk_window_stack::placement(pos2(40.0, 540.0)))
    );
}
