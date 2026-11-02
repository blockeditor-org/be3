use super::*;

#[test]
fn an_edit_coming_back_as_mine_runs_nothing_again() {
    let fixture = Fixture::new();
    let edit = Checklist::set_text(fixture.first, "whole milk");

    fixture.projection.operate(edit.clone());
    fixture.projection.pump();
    assert_eq!(fixture.runs(), [1, 0, 0]);
    assert_eq!(fixture.host.take_content_operations().len(), 1);

    fixture.reset();
    fixture.arrive(&[(edit, true)]);

    assert_eq!(fixture.runs(), [0, 0, 0]);
    assert_eq!(fixture.text(fixture.first), "whole milk");
}
