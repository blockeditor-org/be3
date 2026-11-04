use super::*;

#[test]
fn a_new_window_opens_at_the_size_its_program_picked_and_keeps_it() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let picked = TestWindow {
        size: Vec2::new(640.0, 420.0),
        ..host_window(1, true)
    };
    fixture.windows = vec![picked.clone()];
    fixture
        .test
        .set_watched::<WindowsWatch>(&vec![picked.listed()]);
    fixture.test.run();
    let first = fixture
        .window_rect(1)
        .expect("the window is placed as soon as it is listed");
    assert_eq!(
        first.size(),
        picked.size,
        "it is placed at the size its program picked before any status is heard, so the program is never told another size"
    );

    fixture.settle();
    let tiny = TestWindow {
        size: Vec2::new(3.0, 3.0),
        ..picked.clone()
    };
    fixture.show_windows(&[tiny]);
    assert_eq!(
        fixture.window_rect(1).map(|rect| rect.size()),
        Some(picked.size),
        "a size the program reports later does not resize its window: the dock owns it"
    );

    let unpicked = TestWindow {
        size: Vec2::ZERO,
        ..host_window(2, true)
    };
    fixture.show_windows(&[picked, unpicked]);
    let size = fixture
        .window_rect(2)
        .expect("a window whose program picked no size is placed too")
        .size();
    assert!(
        size.x > 100.0 && size.y > 100.0,
        "it opens at a default size, not shrunk to nothing: {size:?}"
    );
}
