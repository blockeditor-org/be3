use super::*;

#[test]
fn shown_entries_stay_shown_until_hidden_or_gone() {
    let desktop = FakeDesktop::new();
    desktop.add(window(1, "one"));
    desktop.add(window(2, "two"));
    let mut model = Model::new();
    model.update(&desktop, &[]);
    let ids: Vec<EntryId> = model.snapshot().entries.iter().map(|entry| entry.id).collect();
    let shown = |model: &Model| -> Vec<bool> { model.snapshot().entries.iter().map(|entry| entry.shown).collect() };
    assert_eq!(shown(&model), [false, false]);

    model.show(ids[0]);
    model.show(ids[1]);
    assert!(model.update(&desktop, &[]));
    assert_eq!(shown(&model), [true, true]);

    desktop.change(Handle(2), |window| window.visible = false);
    model.update(&desktop, &[Event::Hidden(Handle(2))]);
    desktop.change(Handle(2), |window| window.visible = true);
    model.update(&desktop, &[Event::Shown(Handle(2))]);
    assert_eq!(shown(&model), [true, true]);

    model.hide(ids[0]);
    model.update(&desktop, &[]);
    assert_eq!(shown(&model), [false, true]);

    desktop.remove(Handle(2));
    model.update(&desktop, &[Event::Destroyed(Handle(2))]);
    desktop.add(window(2, "two again"));
    model.update(&desktop, &[Event::Created(Handle(2))]);
    assert_eq!(shown(&model), [false, false]);
}
