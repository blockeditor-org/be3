use super::*;

#[test]
fn displays_come_first_as_display_entries() {
    let display = Entry {
        id: EntryId(1),
        kind: EntryKind::Display,
        title: "Dell U2720Q".to_owned(),
        frame: Rect::new(0, 0, 2000, 1000),
        ..Entry::default()
    };
    let snapshot = snapshot(vec![display.clone(), window(2, Rect::new(0, 0, 100, 100))]);
    let listed = listed(
        &snapshot,
        &[],
        &Shaping::default(),
        &Notes::default(),
        &ProgramNames::default(),
    );
    assert_eq!(listed[0].kind, WindowKind::Display);
    assert_eq!(listed[1].kind, WindowKind::Window);
    let known = known_window(
        &snapshot,
        &display,
        &Shaping::default(),
        &Notes::default(),
        &ProgramNames::default(),
    );
    assert_eq!(known.title, "Dell U2720Q");
    assert_eq!(known.size, Vec2::new(1000.0, 500.0));
}
