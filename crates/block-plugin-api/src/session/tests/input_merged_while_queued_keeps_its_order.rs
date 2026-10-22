use super::*;

#[test]
fn input_merged_while_queued_keeps_its_order() {
    let mut session = running_session();
    let keys: Vec<_> = crate::Key::ALL
        .iter()
        .copied()
        .cycle()
        .take(MAX_QUEUED_MESSAGES * 2)
        .collect();
    for key in &keys {
        session
            .enqueue(input(InputEvent::Key {
                key: *key,
                pressed: true,
                repeat: false,
                text: None,
            }))
            .unwrap();
    }
    assert_eq!(session.queued_message_count(), 1);
    let Some(Message::Input(batch)) = session.next_outbound() else {
        panic!()
    };
    let merged: Vec<_> = batch
        .events
        .iter()
        .map(|event| match event {
            InputEvent::Key { key, .. } => *key,
            _ => panic!(),
        })
        .collect();
    assert_eq!(merged, keys);
}
