use super::*;

#[test]
fn every_sample_of_a_backed_up_stroke_is_kept() {
    let mut session = running_session();
    session.enqueue(batch(vec![pen(1.0, 10)], true)).unwrap();
    session.enqueue(batch(vec![pen(2.0, 20)], true)).unwrap();
    session
        .enqueue(batch(vec![pen(3.0, 30), pen(4.0, 40)], true))
        .unwrap();

    assert_eq!(session.queued_message_count(), 1);
    assert_eq!(
        session.next_outbound(),
        Some(batch(
            vec![pen(1.0, 10), pen(2.0, 20), pen(3.0, 30), pen(4.0, 40)],
            true
        ))
    );
}
