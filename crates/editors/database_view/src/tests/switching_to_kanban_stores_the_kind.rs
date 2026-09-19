use super::*;

#[test]
fn switching_to_kanban_stores_the_kind() {
    let mut fixture = text_editor();

    fixture.test.click("database-view.kind.Kanban");
    fixture.test.run();

    assert_eq!(
        fixture.view.read().unwrap().kind(),
        DatabaseViewKind::Kanban
    );
    fixture.test.snapshot("switching_to_kanban_stores_the_kind");
}
