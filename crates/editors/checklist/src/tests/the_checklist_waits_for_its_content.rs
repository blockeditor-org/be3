use super::*;

#[test]
fn the_checklist_waits_for_its_content() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut editor = BeuiTest::<ChecklistApp>::new(OwnedEditor::new(host, Uuid::new_v4()));
    editor.run();

    assert!(editor.shown("editor.loading"));
    assert!(!editor.shown("checklist.draft"));

    let (_, add) = ChecklistModel::add("buy milk");
    let mut content = ChecklistContent::default();
    content.apply(&add);
    editor.hold(None, content);
    editor.run();

    assert!(!editor.shown("editor.loading"));
    assert!(editor.shown("checklist.draft"));
}
