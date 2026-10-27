use super::*;

#[test]
fn displays_come_first_as_display_entries_and_keep_whether_they_are_shown() {
    let display = Entry {
        id: EntryId(1),
        kind: EntryKind::Display,
        title: "Dell U2720Q".to_owned(),
        frame: Rect::new(0, 0, 2000, 1000),
        shown: true,
        ..Entry::default()
    };
    let snapshot = snapshot(vec![display, window(2, Rect::new(0, 0, 100, 100))]);
    let listed = listed(&snapshot, &[], &Shaping::default(), &Notes::default());
    assert_eq!(listed[0].kind, WindowKind::Display);
    assert_eq!(listed[0].title, "Dell U2720Q");
    assert_eq!(listed[0].size, Vec2::new(1000.0, 500.0));
    assert!(listed[0].shown);
    assert_eq!(listed[1].kind, WindowKind::Window);
    assert!(!listed[1].shown);
}
