use database_types::database_view::{DatabaseViewSort, SortDirection};

use super::*;

#[test]
fn a_card_carried_over_a_sorted_column_opens_its_gap_where_the_sort_puts_it() {
    let Board {
        mut fixture,
        status,
        done,
    } = board(&[("D", false), ("B", true), ("F", true)]);
    let name = fixture.fields[1];
    fixture.edit_view(DatabaseView::set_sort(Some(DatabaseViewSort {
        field_id: name,
        direction: SortDirection::Ascending,
    })));
    fixture.harness.settle();
    let [d, b, f] = [0, 1, 2].map(|index| fixture.row(index));
    let card = |row: ObjectId| format!("database-view.card.{}", row.as_uuid());
    let top = |fixture: &Fixture, row: ObjectId| fixture.harness.rect_of(&card(row)).top();
    let (b_rest, f_rest) = (top(&fixture, b), top(&fixture, f));
    let pitch = f_rest - b_rest;
    let parted = f_rest + pitch;

    let high = fixture.harness.rect_of(&card(b)).center() - Vec2::new(0.0, 20.0);
    let low = fixture.harness.rect_of(&card(f)).center() + Vec2::new(0.0, pitch);
    fixture.lift(fixture.harness.rect_of(&card(d)).center(), high);
    for at in [high, low, high] {
        fixture.harness.hover(at);
        fixture.harness.next_frame();
        fixture.harness.settle();
        assert!(
            (top(&fixture, f) - parted).abs() < 1.0 && (top(&fixture, b) - b_rest).abs() < 0.5,
            "with the pointer at {at:?} the gap opens between B and F, where the sort puts D, \
             so F is at {} (parted {parted}) and B at {} (resting {b_rest})",
            top(&fixture, f),
            top(&fixture, b),
        );
    }
    fixture.harness.record();

    fixture.harness.up(high, PointerButton::Primary);
    let mut frames = Vec::new();
    for _ in 0..40 {
        fixture.harness.run();
        fixture.harness.record();
        frames.push([d, b, f].map(|row| top(&fixture, row)));
    }
    assert_eq!(
        fixture.database().rows.get(d).unwrap().value(status),
        Some(&DatabaseValue::Enum(done))
    );
    assert_eq!(fixture.order(), vec![d, b, f], "the sort decides the order");
    for (frame, [_, b_top, f_top]) in frames.iter().enumerate() {
        assert!(
            (b_top - b_rest).abs() < 0.5 && (f_top - parted).abs() < 0.5,
            "nothing in the column moves after the drop, but in frame {frame} B is at {b_top} \
             and F at {f_top}"
        );
    }
    let [landed, ..] = frames[frames.len() - 1];
    assert!(
        (landed - f_rest).abs() < 1.0,
        "D settles into the gap, where F rested, but is at {landed}"
    );
    let mut previous = frames[0][0];
    for [d_top, ..] in &frames {
        assert!(
            *d_top >= previous - 0.5 && *d_top <= landed + 0.5,
            "D glides straight down into its slot: {frames:?}"
        );
        previous = *d_top;
    }
    fixture
        .harness
        .snapshot("a_card_carried_over_a_sorted_column_opens_its_gap_where_the_sort_puts_it");
}
