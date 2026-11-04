use super::*;

#[test]
fn tiles_past_the_source_share_one_download() {
    let mut worker = TileWorker::spawn(Waker::default());
    let deep = |x, y| TileId { zoom: 16, x, y };
    let source = TileId {
        zoom: SOURCE_MAX_ZOOM,
        x: 10,
        y: 20,
    };

    worker.request(deep(40, 80));
    worker.request(deep(43, 83));

    assert_eq!(deep(43, 83).source(), source);
    assert_eq!(worker.queued, vec![source]);
    assert_eq!(worker.waiting[&source], vec![deep(40, 80), deep(43, 83)]);
    assert_eq!(
        deep(43, 83).window(source),
        Window {
            factor: 4.0,
            offset: [3.0, 3.0],
        }
    );
}
