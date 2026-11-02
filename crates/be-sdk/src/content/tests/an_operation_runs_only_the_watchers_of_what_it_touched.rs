use super::*;

#[test]
fn an_operation_runs_only_the_watchers_of_what_it_touched() {
    let fixture = Fixture::new();

    fixture.arrive(&[(Checklist::set_text(fixture.second, "oat milk"), false)]);

    assert_eq!(fixture.runs(), [0, 1, 0]);
    assert_eq!(fixture.text(fixture.second), "oat milk");

    fixture.reset();
    fixture.arrive(&[(Checklist::add("bread").1, false)]);
    assert_eq!(fixture.runs(), [0, 0, 1]);
}
