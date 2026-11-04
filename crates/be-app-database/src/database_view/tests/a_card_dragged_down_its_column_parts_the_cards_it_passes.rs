use std::time::Duration;

use super::*;

#[test]
fn a_card_dragged_down_its_column_parts_the_cards_it_passes() {
    let Board { mut fixture, .. } = board(&[("A", false), ("B", false), ("C", false), ("D", true)]);
    let [a, b, c, d] = [0, 1, 2, 3].map(|index| fixture.row(index));
    let card = |row: ObjectId| format!("database-view.card.{}", row.as_uuid());
    let rested = fixture.harness.rect_of(&card(a));
    let next = fixture.harness.rect_of(&card(b));
    let passed = fixture.harness.rect_of(&card(c));

    let below = passed.center() + Vec2::new(0.0, 4.0);
    fixture.lift(rested.center(), below);
    fixture.harness.record();
    for _ in 0..3 {
        fixture.harness.advance(Duration::from_millis(50));
        fixture.harness.next_frame();
        fixture.harness.run();
        fixture.harness.record();
    }
    assert_eq!(
        fixture.order(),
        vec![a, b, c, d],
        "nothing is committed while the card is carried"
    );
    let lifted = fixture.harness.rect_of(&card(a));
    assert!(
        (lifted.center().y - below.y).abs() < 1.0,
        "the lifted card follows the pointer"
    );
    let gliding = fixture.harness.rect_of(&card(b));
    assert!(
        gliding.top() < (rested.top() + next.top()) / 2.0,
        "the card it passed glides up into its place, but is at {gliding:?}, from {next:?}, \
         towards {rested:?}"
    );

    fixture.put(below);
    fixture.harness.record();
    fixture
        .harness
        .snapshot("a_card_dragged_down_its_column_parts_the_cards_it_passes");
    assert_eq!(fixture.order(), vec![b, c, a, d]);
    assert_eq!(
        fixture.data().selected_row(),
        None,
        "dropping a card does not open it"
    );
    assert!(fixture.harness.rect_of(&card(a)).top() > fixture.harness.rect_of(&card(c)).top());
}
