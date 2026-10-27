use super::*;

#[test]
fn adding_a_page_links_a_new_text_block_to_the_row() {
    let mut fixture = editor(&[("Title", DatabaseFieldType::String)]);
    let title = fixture.fields[0];
    fixture.set(0, title, DatabaseValue::String("Write the guide".into()));
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::List));
    fixture.settle();

    let row = fixture.database().rows.get_index(0).unwrap().id;
    fixture
        .harness
        .click(format!("database-view.list.row.{}", row.as_uuid()));
    fixture.harness.click("database-view.detail.add-page");

    let page = fixture
        .database()
        .rows
        .get(row)
        .and_then(|row| row.page())
        .expect("the row links its page");
    assert!(fixture.harness.store().created().contains(&page));
    assert!(!fixture.harness.shown("database-view.detail.add-page"));
}
