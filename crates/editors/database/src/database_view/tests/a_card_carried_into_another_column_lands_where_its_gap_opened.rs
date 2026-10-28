use super::*;

#[test]
fn a_card_carried_into_another_column_lands_where_its_gap_opened() {
    let Board {
        mut fixture,
        status,
        done,
        ..
    } = board(&[("A", false), ("B", false), ("C", true), ("D", true)]);
    let [a, b, c, d] = [0, 1, 2, 3].map(|index| fixture.row(index));
    let card = |row: ObjectId| format!("database-view.card.{}", row.as_uuid());
    let before = fixture.harness.applied(Some(fixture.database));
    let first = fixture.harness.rect_of(&card(c));
    let second = fixture.harness.rect_of(&card(d));
    let between = first.center() + (second.center() - first.center()) * 0.5;

    fixture.lift(fixture.harness.rect_of(&card(a)).center(), between);
    fixture.harness.next_frame();
    fixture.harness.run();
    assert!(
        fixture.harness.shown("database-view.card-ghost"),
        "out of its own column the card is carried as a floating preview"
    );
    assert!(
        fixture.harness.rect_of(&card(d)).top() > second.top() + 1.0,
        "the column it is over parts where it would land"
    );

    fixture.put(between);
    assert_eq!(fixture.order(), vec![b, c, a, d]);
    assert_eq!(
        fixture.database().rows.get(a).unwrap().value(status),
        Some(&DatabaseValue::Enum(done))
    );
    assert_eq!(
        fixture.harness.applied(Some(fixture.database)),
        before + 1,
        "the move is one edit"
    );
    let landed = fixture.harness.rect_of(&card(a));
    assert!(landed.left() >= first.left() - 1.0);
    assert!(landed.top() > first.top() && landed.top() < fixture.harness.rect_of(&card(d)).top());
}
