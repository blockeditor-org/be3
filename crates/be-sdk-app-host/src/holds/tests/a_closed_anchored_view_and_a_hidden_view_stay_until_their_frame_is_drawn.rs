use super::*;

#[test]
fn a_closed_anchored_view_and_a_hidden_view_stay_until_their_frame_is_drawn() {
    let mut shown = Shown::new(true);
    let old = layout(&[1, 2]);
    let now = shown.place(&old, layout(&[1]), |screen| screen == ScreenId(2));
    assert_eq!(now, old, "the closed anchored view is still laid out");
    assert_eq!(
        shown.frame_shown(0).closed,
        None,
        "its frame has not been presented"
    );
    assert_eq!(
        shown.presented(Some(7), 6).closed,
        None,
        "its present's pixels have not been drawn"
    );
    assert_eq!(shown.frame_shown(7).closed, Some(layout(&[1])));

    let mut shown = Shown::new(true);
    let holds = |_: &ScreenPlacement| true;
    shown.keep_left_out(&layout(&[1, 2]), &layout(&[1]), holds);
    assert_eq!(shown.kept_placements(), vec![placement(2, 2)]);
    assert_eq!(shown.retained_surfaces(&layout(&[1])), vec![1, 2]);
    shown.keep_left_out(&layout(&[1]), &layout(&[1, 2]), holds);
    shown.release_kept(0, |_| true);
    assert!(
        shown.keeps(ScreenId(2)),
        "shown again, it waits for its new frame"
    );
    assert!(shown.presented(Some(3), 2).kept);
    shown.release_kept(2, |_| true);
    assert!(shown.keeps(ScreenId(2)));
    assert!(shown.frame_shown(3).kept);
    shown.release_kept(3, |_| true);
    assert!(!shown.frame_shown(3).kept, "its new frame has been drawn");

    // In the browser the app's worker shows a present as it makes it, so the kept frame goes
    // once the present is taken in, not when the host draws a frame.
    let mut shown = Shown::new(false);
    shown.keep_left_out(&layout(&[1, 2]), &layout(&[1]), holds);
    assert_eq!(
        shown.kept_placements(),
        vec![placement(2, 2)],
        "the page keeps a hidden view's canvas too"
    );
    shown.keep_left_out(&layout(&[1]), &layout(&[1, 2]), holds);
    shown.release_kept(0, |_| true);
    assert!(shown.keeps(ScreenId(2)), "shown again, it waits for its new frame");
    assert!(shown.presented(Some(3), 0).kept);
    shown.release_kept(0, |_| true);
    assert!(
        !shown.keeps(ScreenId(2)),
        "its new frame is on its canvas once presented"
    );
}
