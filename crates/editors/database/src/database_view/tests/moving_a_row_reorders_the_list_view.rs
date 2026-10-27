use database_types::database_view::FilterCondition;

use super::*;

#[test]
fn moving_a_row_reorders_the_list_view() {
    let mut fixture = viewed(&[
        ("Title", DatabaseFieldType::String),
        ("Status", DatabaseFieldType::Enum),
    ]);
    let (title, status) = (fixture.fields[0], fixture.fields[1]);
    let todo = fixture.option(status, "To do");
    let done = fixture.option(status, "Done");
    for (index, (name, state)) in [("A", todo), ("B", done), ("C", todo), ("D", todo)]
        .into_iter()
        .enumerate()
    {
        fixture.set(index, title, DatabaseValue::String(name.into()));
        fixture.set(index, status, DatabaseValue::Enum(state));
    }
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::List));
    let (_, filter) = DatabaseView::add_filter(
        Some(status),
        FilterCondition::IsNot(DatabaseValue::Enum(done)),
    );
    fixture.edit_view(filter);
    fixture.settle();
    let [a, b, c, d] = [0, 1, 2, 3].map(|index| fixture.row(index));
    let data = fixture.data();
    assert!(data.reorderable.get_untracked());

    data.move_row(d, GroupKey::All, Some(a));
    fixture.settle();
    assert_eq!(fixture.order(), vec![a, d, b, c]);

    data.move_row(c, GroupKey::All, None);
    fixture.settle();
    assert_eq!(
        fixture.order(),
        vec![c, a, d, b],
        "moving to the top of the view puts the row before the first row it shows"
    );

    let top = |row: ObjectId| {
        fixture
            .harness
            .rect_of(&format!("database-view.list.row.{}", row.as_uuid()))
            .top()
    };
    assert!(top(c) < top(a) && top(a) < top(d));
    assert!(
        !fixture
            .harness
            .shown(&format!("database-view.list.row.{}", b.as_uuid()))
    );
}
