use super::*;

#[test]
fn icons_and_processes_are_read_once_until_the_window_shows_again() {
    let desktop = FakeDesktop::new();
    desktop.add(window(1, "one"));
    let icon = Arc::new(Image {
        width: 1,
        height: 1,
        rgba: vec![1, 2, 3, 255],
    });
    desktop.icons.borrow_mut().insert(Handle(1), icon.clone());
    let mut model = Model::new();
    model.update(&desktop, &[]);
    model.update(&desktop, &[Event::Moved(Handle(1)), Event::NameChanged(Handle(1))]);
    assert_eq!(desktop.icon_reads.get(), 1);
    assert_eq!(desktop.process_reads.get(), 1);
    assert_eq!(model.snapshot().entries[0].icon.as_deref(), Some(&*icon));

    let changed = Arc::new(Image {
        width: 1,
        height: 1,
        rgba: vec![9, 9, 9, 255],
    });
    desktop.icons.borrow_mut().insert(Handle(1), changed.clone());
    assert!(model.update(&desktop, &[Event::Shown(Handle(1))]));
    assert_eq!(desktop.icon_reads.get(), 2);
    assert_eq!(model.snapshot().entries[0].icon.as_deref(), Some(&*changed));
}
