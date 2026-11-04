use super::*;

#[test]
fn an_anchored_views_surface_is_kept_while_it_is_open_and_released_once_it_closes() {
    let mut rig = Rig::new(Some(space()));
    let small = Rect::from_min_size(pos2(220.0, 130.0), vec2(60.0, 40.0));
    rig.open(vec![layer(1, 0, small)]);
    rig.draw();
    let first = rig.anchored_screens();
    assert_eq!(first.len(), 1);

    rig.open(vec![layer(1, 0, small.translate(vec2(5.0, 0.0)))]);
    rig.draw();
    assert_eq!(
        rig.anchored_screens(),
        first,
        "a moved view keeps its screen"
    );

    rig.open(vec![layer(
        1,
        0,
        Rect::from_min_size(small.min, vec2(90.0, 40.0)),
    )]);
    rig.draw();
    assert_eq!(
        rig.anchored_screens(),
        first,
        "a resized view keeps its screen and surface"
    );
    assert_eq!(
        rig.shared.seen.borrow().painted.last(),
        Some(&(ViewId(1), [90, 80])),
        "and is painted at its new size"
    );

    rig.open(vec![layer(2, 0, small)]);
    rig.draw();
    let replaced = rig.anchored_screens();
    assert_eq!(replaced.len(), 1, "the closed view's screen is gone");
    assert_ne!(
        replaced[0].1, first[0].1,
        "a newly opened view gets a surface of its own"
    );

    rig.open(Vec::new());
    let closed = rig.draw();
    assert!(Rig::anchored(&closed).is_empty());
    assert!(rig.anchored_screens().is_empty());
}
