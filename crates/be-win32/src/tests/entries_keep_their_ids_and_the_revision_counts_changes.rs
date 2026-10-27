use super::*;

#[test]
fn entries_keep_their_ids_and_the_revision_counts_changes() {
    let desktop = FakeDesktop::new();
    desktop.add(window(1, "one"));
    desktop.add(window(2, "two"));
    let mut model = Model::new();
    assert!(model.update(&desktop, &[]));
    let first = model.snapshot();
    assert_eq!(first.revision, 1);
    assert_eq!(titles(&first), ["one", "two"]);

    assert!(!model.update(&desktop, &[Event::Moved(Handle(1))]));
    assert_eq!(model.snapshot().revision, 1);

    rename(Handle(1), "one, renamed");
    assert!(model.update(&desktop, &[Event::NameChanged(Handle(1))]));
    let renamed = model.snapshot();
    assert_eq!(renamed.revision, 2);
    assert_eq!(renamed.entries[0].id, first.entries[0].id);
    assert_eq!(renamed.entries[0].title, "one, renamed");

    desktop.remove(Handle(1));
    desktop.add(window(3, "three"));
    assert!(model.update(
        &desktop,
        &[Event::Destroyed(Handle(1)), Event::Created(Handle(3))]
    ));
    let replaced = model.snapshot();
    assert_eq!(titles(&replaced), ["two", "three"]);
    assert_eq!(replaced.entries[0].id, first.entries[1].id);
    assert!(replaced.entries[1].id > first.entries[1].id);

    let reused = RawWindow {
        process_id: 999,
        ..window(2, "reused handle")
    };
    desktop.remove(Handle(2));
    desktop.add(reused);
    model.update(&desktop, &[]);
    let after = model.snapshot();
    let entry = after
        .entries
        .iter()
        .find(|entry| entry.handle == Handle(2))
        .unwrap();
    assert_ne!(entry.id, first.entries[1].id);
}
