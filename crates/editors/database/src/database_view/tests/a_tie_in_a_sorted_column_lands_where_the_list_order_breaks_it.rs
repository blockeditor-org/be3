use database_types::database_view::{DatabaseViewSort, SortDirection};

use super::*;

#[test]
fn a_tie_in_a_sorted_column_lands_where_the_list_order_breaks_it() {
    let Board {
        mut fixture,
        status,
        done,
    } = board(&[("C", true), ("A", false), ("D", true)]);
    fixture.edit_view(DatabaseView::set_sort(Some(DatabaseViewSort {
        field_id: status,
        direction: SortDirection::Descending,
    })));
    fixture.harness.settle();
    let [c, a, d] = [0, 1, 2].map(|index| fixture.row(index));
    let card = |row: ObjectId| format!("database-view.card.{}", row.as_uuid());
    let top = |fixture: &Fixture, row: ObjectId| fixture.harness.rect_of(&card(row)).top();
    let (c_rest, d_rest) = (top(&fixture, c), top(&fixture, d));
    let parted = d_rest + (d_rest - c_rest);

    let first = fixture.harness.rect_of(&card(c)).center() - Vec2::new(0.0, 20.0);
    fixture.lift(fixture.harness.rect_of(&card(a)).center(), first);
    fixture.harness.next_frame();
    fixture.harness.settle();
    assert!(
        (top(&fixture, c) - c_rest).abs() < 0.5 && (top(&fixture, d) - parted).abs() < 1.0,
        "the sort's key is the column's own value, so every card in it ties and the list order \
         puts A between C and D wherever the pointer is"
    );

    fixture.put(first);
    fixture.harness.settle();
    assert_eq!(
        fixture.database().rows.get(a).unwrap().value(status),
        Some(&DatabaseValue::Enum(done))
    );
    assert!((top(&fixture, c) - c_rest).abs() < 0.5);
    assert!(
        (top(&fixture, a) - d_rest).abs() < 1.0,
        "A lands in the gap"
    );
    assert!((top(&fixture, d) - parted).abs() < 0.5);
}
