use super::*;

#[test]
fn a_foreign_edit_during_an_edit_in_flight_keeps_both() {
    let fixture = Fixture::new();
    let mine = Checklist::set_done(fixture.first, true);
    fixture.projection.operate(mine.clone());
    fixture.projection.pump();

    fixture.arrive(&[
        (
            Checklist::set_text(fixture.second, "free range eggs"),
            false,
        ),
        (mine, true),
    ]);

    let shown = fixture
        .projection
        .read(|content| {
            content
                .root()
                .items
                .iter()
                .map(|item| (item.text.clone(), item.done))
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(
        shown,
        [
            ("milk".to_owned(), true),
            ("free range eggs".to_owned(), false)
        ]
    );
}
