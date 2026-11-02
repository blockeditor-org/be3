use super::*;

#[test]
fn a_window_entry_becomes_a_host_window_in_its_monitors_points() {
    let mut entry = window(5, Rect::new(100, 100, 900, 700));
    entry.parent = Some(EntryId(4));
    entry.modal = true;
    entry.focused = true;
    entry.raised = 3;
    entry.icon = Some(Arc::new(Image {
        width: 1,
        height: 1,
        rgba: vec![1, 2, 3, 4],
    }));
    entry.origin = Origin {
        program: Some("Accessories/Paint".to_owned()),
        command: vec!["paint.exe".to_owned()],
        launch: Some(9),
    };
    let snapshot = snapshot(vec![entry.clone()]);
    let host = host_window(
        &snapshot,
        &entry,
        &Shaping::default(),
        &Notes::default(),
        &ProgramNames::default(),
    );
    assert_eq!(host.id, HostWindowId(5));
    assert_eq!(host.title, "window 5");
    assert_eq!(host.app_id, "C:\\Programs\\paint.exe");
    assert_eq!(host.parent, Some(HostWindowId(4)));
    assert!(host.modal && host.focused && host.responding);
    assert_eq!(host.size, Vec2::new(400.0, 300.0));
    assert_eq!(host.fullscreen, None);
    assert_eq!(host.raised, 3);
    assert_eq!(host.icon.map(|icon| icon.rgba), Some(vec![1, 2, 3, 4]));
    assert_eq!(host.origin.program.as_deref(), Some("Accessories/Paint"));
    assert_eq!(host.origin.launch, Some(9));
    assert_eq!(host.kind, WindowKind::Window);
    assert!(!host.shown);
}
