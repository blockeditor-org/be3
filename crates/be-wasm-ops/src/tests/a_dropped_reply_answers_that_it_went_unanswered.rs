use super::*;

#[test]
fn a_dropped_reply_answers_that_it_went_unanswered() {
    let mut guest = Connection::new(Side::Guest);
    let mut host = Connection::new(Side::Host);
    let forgotten = guest.start(None, method_id("test#ask"), encode(&()));
    let answered = guest.start(None, method_id("test#ask"), encode(&()));
    deliver(&mut guest, &mut host);
    assert_eq!(events(&mut host).len(), 2);
    drop(Reply::<u32>::new(&host, forgotten));
    assert!(host.has_outbound(), "the dropped reply is owed an answer");
    Reply::<u32>::new(&host, answered).send(&mut host, &3);
    deliver(&mut host, &mut guest);
    assert_eq!(
        events(&mut guest),
        vec![
            Incoming::Completed {
                op: answered,
                outcome: Outcome::returned(&3u32),
            },
            Incoming::Completed {
                op: forgotten,
                outcome: Outcome::Err(OpError::Unanswered),
            },
        ]
    );
}
