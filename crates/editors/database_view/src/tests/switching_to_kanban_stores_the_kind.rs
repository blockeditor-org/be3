use super::*;

#[test]
fn switching_to_kanban_stores_the_kind() {
    let mut fixture = text_editor();

    fixture.harness.editor.click("database-view.kind.Kanban");
    fixture.run();

    assert_eq!(fixture.view_state().kind, DatabaseViewKind::Kanban);
    fixture
        .harness
        .editor
        .snapshot("switching_to_kanban_stores_the_kind");
}
