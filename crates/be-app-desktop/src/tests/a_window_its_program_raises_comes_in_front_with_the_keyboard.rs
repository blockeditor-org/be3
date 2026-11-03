use super::*;

fn in_front(fixture: &Fixture) -> Option<ChildContent> {
    let (one, two) = fixture.window_rect(1).zip(fixture.window_rect(2))?;
    let at = one.intersect(two).center();
    fixture
        .test
        .children()
        .iter()
        .rev()
        .find(|placement| placement.clip.contains_half_open(at))
        .map(|placement| placement.content.clone())
}

#[test]
fn a_window_its_program_raises_comes_in_front_with_the_keyboard() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    assert_eq!(
        in_front(&fixture),
        Some(ChildContent::Window(HostWindowId(2))),
        "the window raised last is in front"
    );
    fixture.focused_windows();

    fixture.raise(1);
    assert_eq!(
        in_front(&fixture),
        Some(ChildContent::Window(HostWindowId(1))),
        "a window its program raises comes in front"
    );
    assert_eq!(
        fixture.focused_windows(),
        [HostWindowId(1)],
        "and is given the keyboard"
    );
}
