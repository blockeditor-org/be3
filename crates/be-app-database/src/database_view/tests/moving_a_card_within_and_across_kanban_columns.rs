use super::*;

#[test]
fn moving_a_card_within_and_across_kanban_columns() {
    let mut fixture = viewed(&[
        ("Status", DatabaseFieldType::Enum),
        ("Name", DatabaseFieldType::String),
    ]);
    let (status, name) = (fixture.fields[0], fixture.fields[1]);
    let todo = fixture.option(status, "To do");
    let done = fixture.option(status, "Done");
    for (index, (title, state)) in [("A", todo), ("B", todo), ("C", todo), ("D", done)]
        .into_iter()
        .enumerate()
    {
        fixture.set(index, status, DatabaseValue::Enum(state));
        fixture.set(index, name, DatabaseValue::String(title.into()));
    }
    fixture.edit_view(DatabaseView::set_group_by(Some(status)));
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::Kanban));
    fixture.settle();
    let [a, b, c, d] = [0, 1, 2, 3].map(|index| fixture.row(index));
    let card = |row: ObjectId| format!("database-view.card.{}", row.as_uuid());
    let data = fixture.data();
    fixture.harness.click(&card(b));
    fixture.settle();
    assert_eq!(data.selected_row(), Some(b));

    data.move_row(c, GroupKey::Option(todo), None);
    fixture.settle();
    assert_eq!(fixture.order(), vec![c, a, b, d]);
    assert!(fixture.harness.rect_of(&card(c)).top() < fixture.harness.rect_of(&card(a)).top());
    assert_eq!(
        data.selected_row(),
        Some(b),
        "the selection follows its row when rows move"
    );

    let before = fixture.harness.applied(Some(fixture.database));
    data.move_row(a, GroupKey::Option(done), Some(d));
    fixture.settle();
    assert_eq!(
        fixture.harness.applied(Some(fixture.database)),
        before + 1,
        "a move across columns is one edit"
    );
    assert_eq!(fixture.order(), vec![c, b, d, a]);
    assert_eq!(
        fixture.database().rows.get(a).unwrap().value(status),
        Some(&DatabaseValue::Enum(done))
    );
    let moved = fixture.harness.rect_of(&card(a));
    let last = fixture.harness.rect_of(&card(d));
    assert!(moved.left() >= fixture.harness.rect_of(&card(c)).right());
    assert!(moved.top() > last.top());
}
