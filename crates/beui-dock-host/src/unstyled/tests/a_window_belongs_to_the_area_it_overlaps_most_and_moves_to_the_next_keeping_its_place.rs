use super::*;

#[test]
fn a_window_belongs_to_the_area_it_overlaps_most_and_moves_to_the_next_keeping_its_place() {
    let _root = RootScope::new();
    let tab = TabId::new;
    let mut state = DockState::new([tab(1)]);
    let floating = Rect::from_min_size(Pos2::new(700.0, 100.0), Vec2::new(400.0, 300.0));
    state.open_window(floating, vec![tab(2)]);
    let layout = DockHostLayout::new();
    layout.restore(state);
    let left = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 800.0));
    let right = Rect::from_min_size(Pos2::new(1000.0, 0.0), Vec2::new(500.0, 400.0));
    layout.set_areas(vec![DockArea::whole(left), DockArea::whole(right)]);
    assert_eq!(
        layout.area_of_window(tab(2)),
        Some(0),
        "a window across two areas belongs to the one it overlaps most"
    );

    layout.step_area(tab(2), false);
    assert_eq!(
        layout.window_rect(tab(2)),
        Some(Rect::from_min_size(
            Pos2::new(1100.0, 50.0),
            Vec2::new(400.0, 300.0)
        )),
        "moved to the next area, it keeps its place in proportion, kept inside the area"
    );
    assert_eq!(layout.area_of_window(tab(2)), Some(1));

    layout.snap_in_area(tab(2), DockSnap::Left);
    layout.step_area(tab(2), false);
    assert_eq!(
        layout.area_of_window(tab(2)),
        Some(0),
        "stepping past the last area comes round to the first"
    );
    assert_eq!(
        layout.window_rect(tab(2)),
        Some(DockSnap::Left.rect(left.shrink(HANDLE_REACH))),
        "a snapped window stays snapped in the area it moves to"
    );
    assert!(layout.unsnap_window(tab(2)));
    assert_eq!(
        layout.area_of_window(tab(2)),
        Some(0),
        "and restored, it goes back to its place in that area"
    );

    layout.set_areas(Vec::new());
    assert_eq!(
        layout.areas(),
        vec![layout.area()],
        "a dock given no areas has its own"
    );
}
