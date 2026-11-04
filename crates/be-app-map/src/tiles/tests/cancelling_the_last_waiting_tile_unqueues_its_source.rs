use super::*;

#[test]
fn cancelling_the_last_waiting_tile_unqueues_its_source() {
    let mut worker = TileWorker::spawn(Waker::default());
    let tile = TileId {
        zoom: 3,
        x: 2,
        y: 5,
    };
    worker.request(tile);

    assert!(worker.cancel(tile));

    assert!(worker.queued.is_empty());
    assert!(worker.waiting.is_empty());
    assert!(!worker.cancel(tile));
}
