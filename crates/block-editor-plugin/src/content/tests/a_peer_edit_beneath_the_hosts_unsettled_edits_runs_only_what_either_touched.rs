use super::*;

#[test]
fn a_peer_edit_beneath_the_hosts_unsettled_edits_runs_only_what_either_touched() {
    let fixture = Fixture::new();
    let done = Checklist::set_done(fixture.first, true);
    fixture.projection.operate(done.clone());
    fixture.projection.pump();
    fixture
        .host
        .push_content_operations(vec![crate::EditorOperation {
            operation: ChecklistContent::encode_operation(&done),
            mine: true,
            unsettled: 1,
            beneath: None,
        }]);
    fixture.projection.pump();
    let renamed = Checklist::set_text(fixture.first, "oat milk");
    fixture.projection.operate(renamed);
    fixture.projection.pump();
    fixture.reset();

    let peer = Checklist::set_text(fixture.second, "free range eggs");
    fixture
        .host
        .push_content_operations(vec![crate::EditorOperation {
            operation: ChecklistContent::encode_operation(&peer),
            mine: false,
            unsettled: 1,
            beneath: Some(vec![ChecklistContent::encode_operation(&done)]),
        }]);
    fixture.projection.pump();

    let [first, second, list] = fixture.runs();
    assert!(first > 0, "the first item's own edits are put back on top");
    assert_eq!(second, 1, "the peer's edit reaches the second item once");
    assert_eq!(list, 0, "nothing reran the whole list");
    assert!(fixture.host.take_content_resend_requests().is_empty());
    assert_eq!(fixture.text(fixture.second), "free range eggs");
    assert_eq!(fixture.text(fixture.first), "oat milk");
    assert!(
        fixture
            .projection
            .read(|content| content.root().items.get(fixture.first).unwrap().done)
            .unwrap()
    );
}
