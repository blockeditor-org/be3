use super::*;

#[test]
fn a_dialog_is_a_modal_child_of_its_disabled_owner() {
    let desktop = FakeDesktop::new();
    let main = window(1, "editor");
    desktop.add(main.clone());
    desktop.add(RawWindow {
        class: "#32770".to_owned(),
        ..owned(2, &main, "Save as")
    });
    let mut model = Model::new();
    model.update(&desktop, &[]);
    let snapshot = model.snapshot();
    let (editor, dialog) = (&snapshot.entries[0], &snapshot.entries[1]);
    assert_eq!(dialog.parent, Some(editor.id));
    assert!(!dialog.modal);
    assert_eq!(editor.parent, None);

    desktop.change(Handle(1), |window| window.enabled = false);
    model.update(&desktop, &[]);
    assert!(model.snapshot().entries[1].modal);
}
