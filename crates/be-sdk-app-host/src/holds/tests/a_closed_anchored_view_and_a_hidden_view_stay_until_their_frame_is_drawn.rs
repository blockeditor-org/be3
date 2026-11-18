use super::*;

#[test]
fn a_closed_anchored_view_and_a_hidden_view_stay_until_their_frame_is_drawn() {
    let mut shown = Shown::new(true);
    let old = layout(&[1, 2]);
    let now = shown.place(&old, layout(&[1]), |screen| screen == ScreenId(2));
    assert_eq!(now, old, "the closed anchored view is still laid out");
    assert_eq!(shown.closed(0), None);
    shown.presented(Some(7));
    assert_eq!(shown.closed(6), None, "its present's pixels have not been drawn");
    assert_eq!(shown.closed(7), Some(layout(&[1])));

    let mut shown = Shown::new(true);
    let holds = |_: &ScreenPlacement| true;
    shown.keep_left_out(&layout(&[1, 2]), &layout(&[1]), holds);
    assert_eq!(shown.kept_placements(), vec![placement(2, 2)]);
    assert_eq!(shown.retained_surfaces(&layout(&[1])), vec![1, 2]);
    shown.keep_left_out(&layout(&[1]), &layout(&[1, 2]), holds);
    shown.release_kept(0, |_| true);
    assert!(shown.keeps(ScreenId(2)), "shown again, it waits for its new frame");
    shown.presented(Some(3));
    shown.release_kept(2, |_| true);
    assert!(shown.keeps(ScreenId(2)));
    shown.release_kept(3, |_| true);
    assert!(!shown.has_kept(), "its new frame has been drawn");

    let mut shown = Shown::new(false);
    shown.keep_left_out(&layout(&[1, 2]), &layout(&[1]), holds);
    assert!(!shown.has_kept(), "a host that holds no textures keeps no frame");
}
