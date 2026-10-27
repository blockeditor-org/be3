use database_types::database_view::{DatabaseViewSort, SortDirection};

use super::*;

#[test]
fn a_sorted_view_refuses_manual_ordering() {
    let mut fixture = viewed(&[
        ("Status", DatabaseFieldType::Enum),
        ("Name", DatabaseFieldType::String),
    ]);
    let (status, name) = (fixture.fields[0], fixture.fields[1]);
    let todo = fixture.option(status, "To do");
    let done = fixture.option(status, "Done");
    for (index, title) in ["A", "B"].into_iter().enumerate() {
        fixture.set(index, status, DatabaseValue::Enum(todo));
        fixture.set(index, name, DatabaseValue::String(title.into()));
    }
    fixture.edit_view(DatabaseView::set_group_by(Some(status)));
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::Kanban));
    fixture.edit_view(DatabaseView::set_sort(Some(DatabaseViewSort {
        field_id: name,
        direction: SortDirection::Descending,
    })));
    fixture.settle();
    let [a, b] = [0, 1].map(|index| fixture.row(index));
    let data = fixture.data();
    assert!(!data.reorderable.get_untracked());

    data.move_row(a, GroupKey::Option(todo), Some(b));
    fixture.settle();
    assert_eq!(fixture.order(), vec![a, b], "a sort decides the order");

    data.move_row(a, GroupKey::Option(done), None);
    fixture.settle();
    assert_eq!(fixture.order(), vec![a, b]);
    assert_eq!(
        fixture.database().rows.get(a).unwrap().value(status),
        Some(&DatabaseValue::Enum(done)),
        "moving to another column still sets its value"
    );

    fixture.edit_view(DatabaseView::set_sort(None));
    fixture.settle();
    assert!(data.reorderable.get_untracked());
}
