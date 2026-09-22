use super::*;

#[test]
fn queue_saturation_preserves_ordered_input() {
    let mut session = running_session();
    let keys: Vec<_> = crate::Key::ALL
        .iter()
        .copied()
        .cycle()
        .take(MAX_QUEUED_MESSAGES)
        .collect();
    for key in &keys {
        session
            .enqueue(input(InputEvent::Key {
                key: *key,
                pressed: true,
                repeat: false,
            }))
            .unwrap();
    }
    assert_eq!(
        session.enqueue(input(InputEvent::Text("overflow".into()))),
        Err(QueueError::Full)
    );
    for expected in keys {
        let Message::Input(batch) = session.next_outbound().unwrap() else {
            panic!()
        };
        let InputEvent::Key { key, .. } = &batch.events[0] else {
            panic!()
        };
        assert_eq!(*key, expected);
    }
}
