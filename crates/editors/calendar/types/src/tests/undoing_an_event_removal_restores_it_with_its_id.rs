use super::*;

#[test]
fn undoing_an_event_removal_restores_it_with_its_id() {
    let (mut content, id) = scheduled();
    let before = content.clone();

    let step = undone(&mut content, Calendar::remove(id));
    assert!(content.root().events.is_empty());

    reverted(&mut content, &step);
    assert_eq!(content, before);
    assert_eq!(
        content
            .root()
            .events
            .get(id)
            .map(|event| event.title.clone()),
        Some("Standup".to_owned())
    );
}
