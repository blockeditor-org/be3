use super::*;
use crate::ops::{Connection, Incoming, Outcome, Side};

#[test]
fn a_watch_answers_its_next_with_the_latest_change() {
    let mut guest = Connection::new(Side::Guest);
    let mut host = Connection::new(Side::Host);
    let mut asked = || {
        let op = guest.start(None, 1, Vec::new());
        for frame in guest.take_outbound() {
            host.receive(&frame).unwrap();
        }
        while host.next_event().is_some() {}
        op
    };
    let first = asked();
    let second = asked();
    let mut watch = Watch::<u32>::default();
    watch.opened(7);
    watch.change(&mut host, |_| 1);
    watch.change(&mut host, |held| held.unwrap_or_default() + 1);
    let replies = [first, second].map(|op| crate::ops::Reply::new(&host, op));
    for reply in replies {
        watch.next(&mut host, reply);
    }
    watch.change(&mut host, |_| 5);
    for frame in host.take_outbound() {
        guest.receive(&frame).unwrap();
    }
    let answers: Vec<_> = std::iter::from_fn(|| guest.next_event())
        .filter_map(|event| match event {
            Incoming::Completed {
                op,
                outcome: Outcome::Ok(bytes),
            } => Some((op, crate::ops::decode::<u32>(&bytes).unwrap())),
            _ => None,
        })
        .collect();
    assert_eq!(
        answers,
        vec![(first, 2), (second, 5)],
        "a change made while nobody waits is kept, folded, for the next ask"
    );
}
