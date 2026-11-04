use super::*;

#[test]
fn an_anchored_view_hears_the_pointer_leave_it() {
    let menu = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0));
    let submenu = Rect::from_min_size(pos2(100.0, 0.0), vec2(100.0, 100.0));
    let layers = vec![shown(1, 1, 1, menu), shown(1, 2, 2, submenu)];
    let pointing = Pointing::default();

    let runs = route(
        &input(vec![mouse(PointerPhase::Move, 50.0, 50.0)]),
        &layers,
        &pointing,
    );
    assert_eq!(targets(&runs), vec![(1, ViewId(1))]);

    let runs = route(
        &input(vec![mouse(PointerPhase::Move, 150.0, 50.0)]),
        &layers,
        &pointing,
    );
    assert_eq!(
        targets(&runs),
        vec![(1, ViewId(1)), (1, ViewId(2))],
        "moving onto another anchored view tells the one it left"
    );
    assert!(matches!(runs[0].1.as_slice(), [Forwarded::Left]));

    let runs = route(&input(vec![Forwarded::Left]), &layers, &pointing);
    assert_eq!(
        targets(&runs),
        vec![(1, ViewId(2))],
        "leaving every anchored view tells the one the pointer was over, \
         so a menu row it hovered stops being highlighted"
    );
    assert!(matches!(runs[0].1.as_slice(), [Forwarded::Left]));

    let runs = route(&input(vec![Forwarded::Left]), &layers, &pointing);
    assert!(runs.is_empty(), "a view hears the pointer leave only once");
}
