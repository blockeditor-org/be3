use std::time::Duration;

use database_types::database_view::{DatabaseViewSort, SortDirection};

use super::*;

#[test]
fn dragging_a_list_row_moves_it_within_and_between_groups() {
    let mut fixture = viewed(&[
        ("Title", DatabaseFieldType::String),
        ("Status", DatabaseFieldType::Enum),
    ]);
    let (title, status) = (fixture.fields[0], fixture.fields[1]);
    let todo = fixture.option(status, "To do");
    let done = fixture.option(status, "Done");
    for (index, (name, state)) in [("A", todo), ("B", todo), ("C", todo), ("D", done)]
        .into_iter()
        .enumerate()
    {
        fixture.set(index, title, DatabaseValue::String(name.into()));
        fixture.set(index, status, DatabaseValue::Enum(state));
    }
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::List));
    fixture.edit_view(DatabaseView::set_group_by(Some(status)));
    fixture.settle();
    let [a, b, c, d] = [0, 1, 2, 3].map(|index| fixture.row(index));
    let line = |row: ObjectId| format!("database-view.list.row.{}", row.as_uuid());

    let start = fixture.harness.rect_of(&line(c));
    let above = fixture.harness.rect_of(&line(a)).center() - Vec2::new(0.0, 4.0);
    fixture.lift(start.center(), above);
    for _ in 0..8 {
        fixture.harness.advance(Duration::from_millis(50));
        fixture.harness.next_frame();
        fixture.harness.run();
    }
    fixture
        .harness
        .snapshot("dragging_a_list_row_moves_it_within_and_between_groups");
    fixture.put(above);
    assert_eq!(fixture.order(), vec![c, a, b, d]);
    assert_eq!(
        fixture.data().selected_row(),
        None,
        "dropping a row does not open it"
    );

    let last = fixture.harness.rect_of(&line(d));
    let under = last.center() + Vec2::new(0.0, last.height() * 0.6);
    fixture.lift(fixture.harness.rect_of(&line(a)).center(), under);
    fixture.put(under);
    assert_eq!(fixture.order(), vec![c, b, d, a]);
    assert_eq!(
        fixture.database().rows.get(a).unwrap().value(status),
        Some(&DatabaseValue::Enum(done)),
        "a row dragged into another group takes that group's value"
    );

    fixture.edit_view(DatabaseView::set_sort(Some(DatabaseViewSort {
        field_id: title,
        direction: SortDirection::Ascending,
    })));
    fixture.settle();
    let before = fixture.harness.applied(Some(fixture.database));
    let first = fixture.harness.rect_of(&line(b));
    fixture.lift(
        first.center(),
        fixture.harness.rect_of(&line(c)).center() - Vec2::new(0.0, 8.0),
    );
    fixture.put(first.center());
    assert_eq!(
        fixture.harness.applied(Some(fixture.database)),
        before,
        "a sorted list does not drag its rows"
    );
}
