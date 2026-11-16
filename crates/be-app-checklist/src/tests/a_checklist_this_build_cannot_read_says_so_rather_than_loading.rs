use super::*;

/// A checklist stored in a form this build cannot read (written by a newer one, or damaged)
/// shows that it could not be read, not a loading pane that never ends.
#[test]
fn a_checklist_this_build_cannot_read_says_so_rather_than_loading() {
    let mut editor = BeuiTest::<ChecklistRoot>::editing(Uuid::new_v4());
    editor
        .store()
        .register(be_type_checklist::registrations());
    editor.store().hold_unreadable::<ChecklistContent>(None);
    editor.run();
    assert!(editor.shown("editor.failed"), "it says it failed");
    assert!(!editor.shown("editor.loading"), "it is not loading");
    editor.snapshot("a_checklist_this_build_cannot_read_says_so_rather_than_loading");
}
