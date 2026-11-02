use super::*;

#[test]
fn an_anchored_view_hears_the_pointer_leave_the_window() {
    let menu = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0));
    let layers = vec![shown(1, 1, 1, menu)];
    let pointing = Pointing::default();

    route(
        &input(vec![mouse(PointerPhase::Move, 50.0, 50.0)]),
        &layers,
        &pointing,
    );
    let runs = route(
        &input(vec![mouse(PointerPhase::Gone, 50.0, 50.0)]),
        &layers,
        &pointing,
    );
    assert_eq!(targets(&runs), vec![(1, ViewId(1))]);
    assert!(
        matches!(runs[0].1.first(), Some(Forwarded::Left)),
        "the pointer leaving the window over a menu tells the menu it left, \
         so the row it hovered stops being highlighted: {:?}",
        runs[0].1
    );

    let runs = route(&input(vec![Forwarded::Left]), &layers, &pointing);
    assert!(runs.is_empty(), "a view hears the pointer leave only once");
}
