use super::*;

#[test]
fn a_foreign_edit_during_an_edit_in_flight_runs_only_what_either_touched() {
    let fixture = Fixture::new();
    fixture
        .projection
        .operate(Checklist::set_done(fixture.first, true));
    fixture.projection.pump();
    fixture.reset();

    fixture.arrive(&[(
        Checklist::set_text(fixture.second, "free range eggs"),
        false,
    )]);

    let [first, second, list] = fixture.runs();
    assert!(first > 0);
    assert_eq!(second, 1);
    assert_eq!(list, 0);
    assert_eq!(fixture.text(fixture.second), "free range eggs");
    assert!(
        fixture
            .projection
            .read(|content| content.root().items.get(fixture.first).unwrap().done)
            .unwrap()
    );
}
