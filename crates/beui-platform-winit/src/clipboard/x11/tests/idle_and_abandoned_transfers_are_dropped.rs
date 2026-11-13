use super::*;

#[test]
fn idle_and_abandoned_transfers_are_dropped() {
    let start = Instant::now();
    let mut transfers = Transfers::default();
    let data: Arc<[u8]> = vec![7; CHUNK * 3].into();
    transfers.start(1, 5, 9, Arc::clone(&data), start);
    transfers.start(2, 5, 9, Arc::clone(&data), start);
    transfers.start(3, 5, 9, Arc::clone(&data), start);

    transfers.forget(3);
    assert!(
        !transfers.names(3),
        "a requestor that went away is forgotten"
    );

    let later = start + IDLE / 2;
    assert!(transfers.next(2, 5, later).is_some());
    assert_eq!(
        transfers.expire(later),
        [],
        "nothing has been idle long enough"
    );

    let stale = start + IDLE;
    assert_eq!(
        transfers.expire(stale),
        [1],
        "the requestor that stopped taking pieces is dropped, and its events no longer heard"
    );
    assert!(transfers.names(2), "one still taking pieces stays");
    assert_eq!(transfers.expire(later + IDLE), [2]);
}
