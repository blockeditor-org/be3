use super::*;

/// Until the TileJSON says what each tile is cut from, the tiles asked for wait, and one cancelled
/// meanwhile is forgotten; reading it queues the rest by their source.
#[test]
fn tiles_asked_for_before_the_source_is_read_wait_for_it() {
    let mut worker = TileWorker::spawn(Waker::default());
    let deep = |x, y| TileId { zoom: 16, x, y };
    worker.request(deep(40, 80));
    worker.request(deep(43, 83));
    worker.request(deep(0, 0));

    assert!(worker.cancel(deep(0, 0)));
    assert!(worker.queued.is_empty());

    worker.read(TileSource {
        template: "https://tiles.openfreemap.org/planet/test/{z}/{x}/{y}.pbf".to_owned(),
        max_zoom: MAX_ZOOM,
    });

    let source = TileId {
        zoom: MAX_ZOOM,
        x: 10,
        y: 20,
    };
    assert_eq!(worker.queued, vec![source]);
    assert_eq!(worker.waiting[&source], vec![deep(40, 80), deep(43, 83)]);
}
