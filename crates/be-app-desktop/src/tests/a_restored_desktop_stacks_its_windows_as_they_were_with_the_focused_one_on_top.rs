use super::*;

fn front_at(fixture: &Fixture, at: Pos2) -> Option<ChildContent> {
    fixture
        .test
        .children()
        .iter()
        .rev()
        .find(|placement| placement.clip.contains_half_open(at))
        .map(|placement| placement.content.clone())
}

fn focused(window: HostWindow) -> HostWindow {
    HostWindow {
        focused: true,
        ..window
    }
}

#[test]
fn a_restored_desktop_stacks_its_windows_as_they_were_with_the_focused_one_on_top() {
    let mut before = Fixture::new();
    before.settle();
    before.show_windows(&[foot(1), focused(notes(6))]);
    before.show_windows(&[focused(foot(1)), notes(6)]);
    let overlap = before
        .window_rect(1)
        .zip(before.window_rect(6))
        .map(|(foot, notes)| foot.intersect(notes))
        .expect("both windows are placed");
    assert!(overlap.is_positive(), "the windows overlap");
    assert_eq!(
        front_at(&before, overlap.center()),
        Some(ChildContent::Window(HostWindowId(1))),
        "the window the host focused last is in front"
    );
    let saved = before.saved();

    let mut adopted = Fixture::resumed(saved.clone());
    adopted.show_windows(&[foot(1), notes(6)]);
    assert_eq!(
        front_at(&adopted, overlap.center()),
        Some(ChildContent::Window(HostWindowId(1))),
        "a desktop started again over the same windows stacks them as they were"
    );
    adopted.test.clear_calls();
    adopted.show_windows(&[foot(1), focused(notes(6))]);
    assert_eq!(
        front_at(&adopted, overlap.center()),
        Some(ChildContent::Window(HostWindowId(6))),
        "and the window focused next comes in front"
    );

    let mut resumed = Fixture::resumed(saved);
    resumed.test.settle();
    resumed
        .test
        .snapshot("a_restored_desktop_shows_the_focused_placeholder_on_top");
    resumed.show_windows(&[notes(6)]);
    resumed.show_windows(&[notes(6), foot(1)]);
    assert_eq!(
        front_at(&resumed, overlap.center()),
        Some(ChildContent::Window(HostWindowId(1))),
        "windows that come back in another order still stack as they were saved"
    );
}
