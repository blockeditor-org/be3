use database_types::database_view::{DatabaseViewSort, SortDirection};

use super::*;

#[test]
fn a_sorted_kanban_moves_cards_between_columns_but_not_within_one() {
    let Board {
        mut fixture,
        status,
        done,
        ..
    } = board(&[("A", false), ("B", false), ("C", true)]);
    let name = fixture.fields[1];
    fixture.edit_view(DatabaseView::set_sort(Some(DatabaseViewSort {
        field_id: name,
        direction: SortDirection::Descending,
    })));
    fixture.settle();
    let [a, b, c] = [0, 1, 2].map(|index| fixture.row(index));
    let card = |row: ObjectId| format!("database-view.card.{}", row.as_uuid());
    let top = fixture.harness.rect_of(&card(b));
    let under = fixture.harness.rect_of(&card(a));
    assert!(top.top() < under.top(), "the sort puts B above A");
    let before = fixture.harness.applied(Some(fixture.database));

    let below = under.center() + Vec2::new(0.0, 8.0);
    fixture.lift(top.center(), below);
    fixture.harness.next_frame();
    fixture.harness.run();
    assert!(
        fixture.harness.shown("database-view.card-ghost"),
        "a card of a sorted column is carried as a floating preview"
    );
    assert_eq!(
        fixture.harness.rect_of(&card(a)).top(),
        under.top(),
        "the cards it passes stay where the sort puts them"
    );
    fixture.put(below);
    assert_eq!(
        fixture.harness.applied(Some(fixture.database)),
        before,
        "a drop in its own sorted column changes nothing"
    );
    assert_eq!(fixture.order(), vec![a, b, c]);

    let other = fixture.harness.rect_of(&card(c));
    let at = other.center() + Vec2::new(0.0, other.height());
    fixture.lift(fixture.harness.rect_of(&card(a)).center(), at);
    fixture.put(at);
    assert_eq!(
        fixture.database().rows.get(a).unwrap().value(status),
        Some(&DatabaseValue::Enum(done)),
        "a card still moves to another column, which sets its status"
    );
    assert_eq!(fixture.order(), vec![a, b, c], "the sort decides the order");
    assert!(
        fixture.harness.rect_of(&card(a)).top() > fixture.harness.rect_of(&card(c)).top(),
        "it lands where the sort puts it"
    );
}
