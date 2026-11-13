use super::*;

/// A read between frames takes what the replica sent, so the next frame no longer finds the
/// block waiting: a change to the edits the server lacks, which touches no content, still asks
/// for the pump that tells whoever follows them.
#[test]
fn lacking_edits_a_read_takes_are_announced_at_the_next_pump() {
    let fixture = Fixture::new();
    let due = Rc::new(Cell::new(0));
    let asked = Rc::clone(&due);
    fixture.projection.on_due(move || asked.set(asked.get() + 1));
    let heard = Rc::new(Cell::new(0));
    let told = Rc::clone(&heard);
    let _following = fixture.projection.on_lacking(move || told.set(told.get() + 1));
    fixture.projection.follow_lacking();
    let (_, add) = Checklist::add("bread");
    fixture.host.receive_from_replica(
        None,
        crate::ToEditor::Lacking {
            reset: true,
            lacking: vec![be_sdk_type_guest::editor::Lacked {
                id: 1,
                operation: ChecklistContent::encode_operation(&add),
                failed: false,
            }],
            held: Vec::new(),
        },
    );
    assert!(fixture.projection.ready().is_ok());
    assert!(fixture.host.updated_content().is_empty());
    assert_eq!(due.get(), 1, "the read asks for a pump");
    fixture.projection.pump();
    assert_eq!(heard.get(), 1);
    assert_eq!(fixture.projection.lacking().len(), 1);
}
