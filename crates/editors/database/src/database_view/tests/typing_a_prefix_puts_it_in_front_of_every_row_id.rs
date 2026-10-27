use super::*;

#[test]
fn typing_a_prefix_puts_it_in_front_of_every_row_id() {
    let mut fixture = text_editor();
    let title = fixture.fields[0];
    fixture.set(0, title, DatabaseValue::String("Fix the login".into()));
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::List));
    fixture.settle();

    fixture.harness.click("database-view.display");
    fixture.harness.click("database-view.prefix");
    fixture.run();
    fixture.harness.type_text("BE");

    let database = fixture.database();
    assert_eq!(database.prefix, "BE");
    let row = database.rows.get_index(0).unwrap().id;
    assert!(database.short_id(row).starts_with("BE-"));
}
