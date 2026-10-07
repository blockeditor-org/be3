use super::*;

#[test]
fn a_block_shown_on_the_desktop_opens_in_its_own_window() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let (first, second) = (Uuid::new_v4(), Uuid::new_v4());

    fixture.host.show_block(first, SHOWN_TYPE, None);
    fixture.settle();
    fixture.host.show_block(second, SHOWN_TYPE, None);
    fixture.settle();

    let placed = fixture.placed_blocks();
    let rect_of = |id: Uuid| {
        placed
            .iter()
            .find(|(block, _, _)| *block == id)
            .map(|(_, view, rect)| (*view, *rect))
            .unwrap_or_else(|| panic!("{id} is placed: {placed:?}"))
    };
    let (first_view, first_rect) = rect_of(first);
    let (second_view, second_rect) = rect_of(second);
    assert!(
        first_view.is_some_and(|view| view != first),
        "an ordinary block gets a view of its own"
    );
    assert_ne!(first_view, second_view);
    assert_ne!(
        first_rect.min, second_rect.min,
        "each block floats in a window of its own"
    );
}
