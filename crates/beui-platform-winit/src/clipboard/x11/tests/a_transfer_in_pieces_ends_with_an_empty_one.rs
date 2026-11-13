use super::*;

#[test]
fn a_transfer_in_pieces_ends_with_an_empty_one() {
    let now = Instant::now();
    let mut transfers = Transfers::default();
    let data: Arc<[u8]> = vec![7; CHUNK + 3].into();
    let (requestor, property, kind) = (10, 20, 30);
    transfers.start(requestor, property, kind, Arc::clone(&data), now);
    // A second transfer to the same requestor, under another property.
    transfers.start(requestor, property + 1, kind, Arc::clone(&data), now);

    let ranges = std::iter::from_fn(|| transfers.next(requestor, property, now))
        .map(|piece| (piece.range, piece.last, piece.kind))
        .collect::<Vec<_>>();
    assert_eq!(
        ranges,
        [
            (0..CHUNK, false, kind),
            (CHUNK..CHUNK + 3, false, kind),
            (CHUNK + 3..CHUNK + 3, true, kind),
        ]
    );
    assert!(
        transfers.names(requestor),
        "the requestor's other transfer still needs its events"
    );
    assert_eq!(transfers.next(requestor, property, now), None);
}
