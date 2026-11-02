use super::*;

#[test]
fn the_layout_notice_shows_above_the_desktops_windows() {
    let mut first = Fixture::new();
    first.settle();
    first.show_windows(&[host_window(1, true)]);
    let mut layout = first
        .saved()
        .root()
        .state("layout")
        .cloned()
        .expect("the layout is saved");
    layout.bytes.truncate(layout.bytes.len() / 2);
    let mut saved = EditorView::document(None);
    let edit = saved
        .root()
        .set_state("layout", Some(&layout), 1, Uuid::nil());
    saved.apply(&edit);

    let mut fixture = Fixture::holding(saved);
    fixture.settle();
    fixture.show_windows(&[host_window(2, true)]);
    fixture.test.show_on_displays(&[(
        Rect::from_min_size(Default::default(), Vec2::new(800.0, 600.0)),
        1.0,
    )]);
    fixture.settle();

    let window = fixture
        .test
        .desk_windows()
        .first()
        .copied()
        .expect("the program's window is a window of its own on the display");
    let reset = fixture
        .test
        .desk_rects_of("workspace.layout.reset")
        .first()
        .copied()
        .expect("the notice is shown on the display");
    let card = fixture
        .test
        .desk_rects_of("workspace.layout")
        .first()
        .copied()
        .expect("the notice's card carries the dialog's id");
    assert!(
        card.contains_rect(reset),
        "the card holds its buttons: {card:?} {reset:?}"
    );
    let over = reset.intersect(window);
    assert!(
        over.is_positive(),
        "the notice's button lies over the window: {reset:?} {window:?}"
    );
    fixture
        .test
        .desk_snapshot("the_layout_notice_above_a_window", 0);

    fixture.test.click(over.center());
    fixture.settle();
    assert!(
        !fixture.test.shown("workspace.layout.reset"),
        "a press on the notice reaches it rather than the window beneath"
    );
}
