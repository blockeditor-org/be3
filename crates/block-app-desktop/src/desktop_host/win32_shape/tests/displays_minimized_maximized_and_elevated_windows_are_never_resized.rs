use super::*;

#[test]
fn displays_minimized_maximized_and_elevated_windows_are_never_resized() {
    let display = Entry {
        kind: EntryKind::Display,
        ..window(1, Rect::new(0, 0, 2000, 1000))
    };
    let minimized = Entry {
        minimized: true,
        ..window(2, Rect::new(0, 0, 100, 100))
    };
    let maximized = Entry {
        maximized: true,
        ..window(3, Rect::new(0, 0, 2000, 960))
    };
    let elevated = Entry {
        control: Control::Elevated,
        ..window(4, Rect::new(0, 0, 100, 100))
    };
    let covering = window(5, Rect::new(0, 0, 2000, 1000));
    let snapshot = snapshot(vec![display, minimized, maximized, elevated, covering]);
    let mut shaping = Shaping::default();
    shaping.observe(&snapshot);
    for id in 1..=6 {
        assert_eq!(
            shaping.placed(&snapshot, EntryId(id), Vec2::new(300.0, 200.0)),
            None
        );
    }
}
